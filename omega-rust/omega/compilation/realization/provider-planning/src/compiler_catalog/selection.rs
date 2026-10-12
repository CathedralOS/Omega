//! Automatic primitive choice is issued here, never by candidate uniqueness.
//!
//! The original loaded toolchain units must equal the bundled catalog units.
//! Their canonical consumed-source identities, exact declaration handles,
//! complete closed signature, satisfaction edge and selected target origin
//! travel with the execution meaning. The enclosing selected carrier retains
//! the complete plan and original typed owner; the digest only joins that plan.

use super::SelectedCompilerIntrinsicExecutionIdentity;
use super::execution_identities::{execution_identity, requirement_execution};
use crate::{
    DerivedProviderPlan, IntrinsicRequirement, ProviderSelectionProvenance,
    SelectedProviderPlanWithProvenance, SelectedTargetMachineOrigin,
};
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanDigest};
use symbols::SymbolHandle;

/// A private-issued automatic choice under the enclosing compilation's original
/// typed owner. It is distinct from an authored Build/default selection and
/// from accepted hosted-service settlement. Neither a CompilerIntrinsic tag
/// nor a similarly named source declaration can construct this receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerCatalogChoice {
    plan: ProviderPlanDigest,
    requirement: SymbolHandle,
    realization: SymbolHandle,
    origin: SelectedTargetMachineOrigin,
    requirement_source: [u8; 32],
    realization_source: [u8; 32],
    execution: CompilerIntrinsicExecutionIdentity,
}

impl CompilerCatalogChoice {
    pub fn requirement(&self) -> SymbolHandle {
        self.requirement
    }
    pub fn realization(&self) -> SymbolHandle {
        self.realization
    }
    pub fn target(&self) -> &str {
        &self.origin.target
    }
    pub fn source_identities(&self) -> ([u8; 32], [u8; 32]) {
        (self.requirement_source, self.realization_source)
    }

    /// Consume the execution issued for this exact retained plan and row.
    /// This is an association query, not a replay of the issuer's source check.
    pub fn row_execution(
        &self,
        plan: &ProviderPlan,
        requirement: SymbolHandle,
        realization: SymbolHandle,
        selected_target: Option<&str>,
    ) -> Result<CompilerIntrinsicExecutionIdentity, Diagnostic> {
        if plan.identity_digest() != self.plan
            || requirement != self.requirement
            || realization != self.realization
            || selected_target != Some(self.target())
            || plan.target != self.target()
        {
            return Err(Diagnostic::error(
                "automatic compiler catalog choice lost its exact plan, declaration or target association",
            ));
        }
        Ok(self.execution)
    }
}

pub(crate) fn retain_catalog_choices(
    context: crate::ProviderPlanningContext<'_>,
    candidates: &[DerivedProviderPlan],
    origins: &[SelectedTargetMachineOrigin],
    selected_target: Option<&str>,
    selected: &mut Vec<SelectedProviderPlanWithProvenance>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(selected_target) = selected_target else {
        return Ok(());
    };
    let typed = context.typed;
    let mut issued: Vec<(usize, CompilerCatalogChoice)> = Vec::new();
    let mut sources = CatalogSources::default();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate
            .provenance
            .row_requirements
            .iter()
            .chain(&candidate.provenance.row_realizations)
            .any(|symbol| !context.contains(*symbol))
        {
            continue;
        }
        // Explicit/default choices take precedence. They do not enter the
        // automatic issuer, even when the candidate is a catalog declaration.
        if selected
            .iter()
            .any(|plan| plan.derived.provenance.schema == candidate.provenance.schema)
        {
            continue;
        }
        let Some(choice) =
            issue_catalog_choice(typed, candidate, origins, selected_target, &mut sources)
                .map_err(|error| vec![error])?
        else {
            continue;
        };
        if issued
            .iter()
            .any(|(_, prior)| prior.requirement == choice.requirement)
        {
            return Err(vec![Diagnostic::error(
                "automatic compiler catalog requirement has conflicting original realization receipts",
            )]);
        }
        issued.push((index, choice));
    }
    selected.extend(
        issued
            .into_iter()
            .map(|(index, choice)| SelectedProviderPlanWithProvenance {
                derived: candidates[index].clone(),
                selected_by: ProviderSelectionProvenance::CompilerCatalog(choice),
            }),
    );
    Ok(())
}

fn issue_catalog_choice(
    typed: &TypedTrees,
    candidate: &DerivedProviderPlan,
    origins: &[SelectedTargetMachineOrigin],
    selected_target: &str,
    sources: &mut CatalogSources,
) -> Result<Option<CompilerCatalogChoice>, Diagnostic> {
    let plan = &candidate.plan;
    let ([row], [method], [requirement_symbol], [realization_symbol]) = (
        plan.rows.as_slice(),
        plan.schema.methods.as_slice(),
        candidate.provenance.row_requirements.as_slice(),
        candidate.provenance.row_realizations.as_slice(),
    ) else {
        return Ok(None);
    };
    let ProviderBinding::CompilerIntrinsic { machine, .. } = &row.binding else {
        return Ok(None);
    };
    let Some(requirement) = IntrinsicRequirement::by_symbol(typed, *requirement_symbol) else {
        return Ok(None);
    };
    if candidate.provenance.schema.symbol() != requirement.symbol
        || plan.target != selected_target
        || !requirement.plan_row_binds(plan, method, row)
    {
        return Ok(None);
    }
    let Some(requirement_source) = sources.authenticate(
        typed,
        requirement.symbol,
        "float_operations.omg",
        BUNDLED_REQUIREMENTS,
    )?
    else {
        return Ok(None);
    };
    let expected_identity = format!("toolchain::{}::{}", requirement.namespace, requirement.name);
    if typed
        .normalized_hermetic_symbol_identity(requirement.symbol)
        .ok()
        .as_deref()
        != Some(expected_identity.as_str())
        || !closed_requirement_scope(typed, &requirement)
    {
        return Ok(None);
    }
    let Some(realization) = typed.machine(*realization_symbol) else {
        return Ok(None);
    };
    let Some((relative, bundled)) = bundled_realizations(selected_target) else {
        return Ok(None);
    };
    let Some(realization_source) =
        sources.authenticate(typed, realization.symbol, relative, bundled)?
    else {
        return Ok(None);
    };
    let mut original_origins = origins
        .iter()
        .filter(|origin| origin.machine == realization.symbol);
    let Some(origin) = original_origins.next() else {
        return Ok(None);
    };
    if original_origins.next().is_some()
        || origin.target != selected_target
        || realization
            .authored_target
            .as_ref()
            .map(|target| target.as_str())
            != Some(selected_target)
        || realization.body_is_present
        || !realization.has_bodyless_signature()
        || !realization.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(realization).is_empty()
        || !realization.conformance_bounds.is_empty()
        || realization.blocks
        || realization.suspends
        || typed.machine_callable_identity_text(realization).as_deref() != Some(machine.as_str())
    {
        return Ok(None);
    }
    let [entry] = typed.machine_states(realization) else {
        return Ok(None);
    };
    let parameters = typed.state_parameters(entry);
    if parameters.len() != requirement.parameters.len()
        || !same_type(typed, entry.return_type, requirement.return_type)
        || !parameters.iter().zip(requirement.parameters).all(|(actual, formal)| {
            !actual.is_const && !actual.is_mutable && actual.relevance == formal.relevance
                && same_type(typed, actual.type_reference, formal.type_reference)
        })
        || !typed.machine_trait_conformances(realization).iter().any(|clause| {
            checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(typed, realization, clause)
                .is_some_and(|operator| operator.symbol == requirement.symbol)
                || matches!(checked_trees::typed_trees::machine::resolve_satisfied_declaration(typed, realization, clause),
                    Some(checked_trees::typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(original))
                        if original.symbol == requirement.symbol)
        })
    { return Ok(None); }
    let Some(classification) = requirement_execution(typed, &requirement)? else {
        return Ok(None);
    };
    let Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(execution)) =
        execution_identity(typed, &requirement, classification)?
    else {
        return Ok(None);
    };
    Ok(Some(CompilerCatalogChoice {
        plan: plan.identity_digest(),
        requirement: requirement.symbol,
        realization: realization.symbol,
        origin: origin.clone(),
        requirement_source,
        realization_source,
        execution,
    }))
}

fn same_type(
    typed: &TypedTrees,
    left: checked_trees::typed_trees::types::TypeReferenceHandle,
    right: checked_trees::typed_trees::types::TypeReferenceHandle,
) -> bool {
    let left = typed.normalized_type_identity(left);
    let right = typed.normalized_type_identity(right);
    left.is_valid() && right.is_valid() && left == right
}

fn closed_requirement_scope(typed: &TypedTrees, requirement: &IntrinsicRequirement<'_>) -> bool {
    if requirement.parameters.iter().any(|parameter| {
        parameter.is_const
            || parameter.is_mutable
            || parameter.relevance != language_core::BindingRelevance::Relevant
    }) {
        return false;
    }
    if let Some(machine) = typed.machine(requirement.symbol) {
        return machine.lifetime_parameters.is_empty()
            && typed.machine_type_parameters(machine).is_empty()
            && machine.conformance_bounds.is_empty()
            && !machine.blocks
            && !machine.suspends;
    }
    requirement.as_operator().is_some_and(|operator| {
        operator.lifetime_parameters.is_empty() && operator.type_parameters.is_empty()
    })
}

/// Invocation scratch, keyed by the original source-ID under this one typed
/// owner. Whole immutable unit custody is admitted once, then shared by rows.
#[derive(Default)]
struct CatalogSources {
    units: Vec<(source::SourceId, &'static str, [u8; 32])>,
}

impl CatalogSources {
    fn authenticate(
        &mut self,
        typed: &TypedTrees,
        symbol: SymbolHandle,
        relative: &'static str,
        bundled: &[u8],
    ) -> Result<Option<[u8; 32]>, Diagnostic> {
        let Some(source) = typed
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| typed.symbols.source_file(span))
        else {
            return Ok(None);
        };
        if source.origin != source::SourceOrigin::Toolchain
            || source.package_identity.is_some()
            || source.path.strip_prefix(&source.package_root).ok()
                != Some(std::path::Path::new(relative))
        {
            return Ok(None);
        }
        if let Some((_, _, digest)) = self
            .units
            .iter()
            .find(|(source_id, path, _)| *source_id == source.source_id && *path == relative)
        {
            return Ok(Some(*digest));
        }
        if source.source.as_bytes() != bundled {
            return Ok(None);
        }
        let digest =
            package_compilation::toolchain_source_identity_digest(source).map_err(|errors| {
                Diagnostic::error(format!(
                    "compiler catalog source custody is unavailable: {errors:?}"
                ))
            })?;
        self.units.push((source.source_id, relative, digest));
        Ok(Some(digest))
    }
}

const BUNDLED_REQUIREMENTS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/float_operations.omg"
));

fn bundled_realizations(target: &str) -> Option<(&'static str, &'static [u8])> {
    macro_rules! source {
        ($target:literal) => {
            (
                concat!("targets/", $target, "/float_impl.omg"),
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/targets/",
                    $target,
                    "/float_impl.omg"
                ))
                .as_slice(),
            )
        };
    }
    Some(match target {
        "macos_arm64" => source!("macos_arm64"),
        "macos_x86_64" => source!("macos_x86_64"),
        "linux_arm64" => source!("linux_arm64"),
        "linux_x86_64" => source!("linux_x86_64"),
        "windows_x86_64" => source!("windows_x86_64"),
        _ => return None,
    })
}
