//! Retired alternate target-demand entry. The active Product driver settles providers
//! against its checked publication, and `CompletedProductSource::prepare_provider_demand`
//! reads the result for the selected deployment.

/// What a product of one target reads from its checked source to prepare
/// that target's supply.
pub struct TargetSupplySource<'a> {
    pub numeric_support: Option<&'a language_core::float_operations::NumericTargetCapabilities>,
    /// The Product's source and supplied roster, paired by their judgment.
    pub publication: &'a crate::SourcePublication,
    /// The evaluated Build's provider selections and the target packages'
    /// defaults.
    pub build: &'a [crate::compile_product::provider_declarations::ProviderSelection],
    pub native_bindings: &'a [crate::provider_choice_inputs::NativeBuildSupply],
    pub defaults: &'a [crate::compile_product::provider_declarations::TargetProviderDefaults],
    /// The selected deployment, or no deployment for targetless checking.
    pub target: Option<&'a str>,
}

/// Prepares what one target must supply: the requirements its entries reach
/// through the providers it chooses, each with the supplies declared for it.
///
/// Nothing is judged here. A requirement whose chosen provider the accepted
/// module does not hold is refused by name: the walk could not continue
/// through that provider, so what it reaches is unknown, and a product made
/// from the module would call a provider nobody judged.
pub fn prepare_supplied_target(
    source: TargetSupplySource<'_>,
) -> Result<PreparedTargetSupply, Vec<Diagnostic>> {
    let accepted = source.publication.accepted()?;
    let supply = ModuleDemand::read(accepted, source.publication.roster()?)?.prepare(
        accepted,
        source.build,
        source.native_bindings,
        source.defaults,
        source.target,
        source.numeric_support,
    )?;
    let unjudged = supply
        .requirements()
        .iter()
        .filter(|requirement| requirement.realization.is_none())
        .filter_map(|requirement| {
            let chosen = requirement.chosen()?;
            let selection = chosen.checked()?;
            Some(Diagnostic::error(format!(
                "no product route yet: target preparation for `{}`: `{}` is supplied by `{}`, which the accepted module does not hold as a judged provider",
                source.target.unwrap_or("<unscoped>"),
                requirement.reached.name,
                selection.provider_type.authored_path,
            )))
        })
        .collect::<Vec<_>>();
    if !unjudged.is_empty() {
        return Err(unjudged);
    }
    Ok(supply)
}
