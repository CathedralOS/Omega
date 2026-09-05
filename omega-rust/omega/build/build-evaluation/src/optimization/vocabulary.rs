use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OptimizationBuildVocabulary {
    LegacyWithoutField,
    Canonical,
}

/// Classify the selected Build declaration before constructing its interpreter
/// argument. BuildTimeValue intentionally carries no nominal symbol, so doing
/// this after evaluation would let an authored lookalike cross the boundary.
pub(super) fn classify(typed: &TypedTrees) -> Result<OptimizationBuildVocabulary, Vec<Diagnostic>> {
    let named_builds = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name.as_str() == "Build")
        .collect::<Vec<_>>();
    let toolchain_builds = named_builds
        .iter()
        .copied()
        .filter(|definition| {
            super::super::is_exact_toolchain_build_prelude_data(typed, definition.symbol, "Build")
        })
        .collect::<Vec<_>>();
    let builds = if toolchain_builds.is_empty() {
        &named_builds
    } else {
        &toolchain_builds
    };
    let [build] = builds.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "build-machine evaluation requires exactly one `Build` data declaration, found {}",
            builds.len()
        ))]);
    };
    let optimization_fields = typed
        .data_members(build)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == "optimizations" =>
            {
                Some(field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [] = optimization_fields.as_slice() else {
        let [field] = optimization_fields.as_slice() else {
            return Err(vec![Diagnostic::error(
                "Build declares more than one `optimizations` field",
            )]);
        };
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            return Err(vec![Diagnostic::error(format!(
                "Build.optimizations must have the exact toolchain `Optimizations` type, got `{}`",
                typed.display_type_reference_with_constraints(field.type_reference)
            ))]);
        };
        if !super::super::is_exact_toolchain_build_prelude_data(typed, *symbol, "Optimizations") {
            return Err(vec![Diagnostic::error(format!(
                "Build.optimizations must have the exact toolchain `Optimizations` type, got `{}`",
                typed.display_type_reference_with_constraints(field.type_reference)
            ))]);
        }
        if !super::super::is_exact_toolchain_build_prelude_data(typed, build.symbol, "Build") {
            return Err(vec![Diagnostic::error(
                "Build.optimizations is reserved to the toolchain-provided Build vocabulary",
            )]);
        }
        return Ok(OptimizationBuildVocabulary::Canonical);
    };
    Ok(OptimizationBuildVocabulary::LegacyWithoutField)
}
