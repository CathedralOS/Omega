//! Source-owned provider selection inputs retain the exact authored origin.
//! Native planning receives the closed canonical selections instead.
use semantic_vocabulary::PackageKeyIdentity;
use semantic_vocabulary::source_selections::{
    CompositionMode, NativeBindingProducer, ProductTarget,
};
use symbols::SymbolHandle;

/// Exact prepared application coordinate. Only the source owner can join it
/// to its private arena; native consumers use the full canonical identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderApplicationId {
    index: u32,
    generation: u32,
}

impl ProviderApplicationId {
    pub const fn from_parts(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
    pub const fn arena_index(self) -> u32 {
        self.index
    }
    pub const fn generation(self) -> u32 {
        self.generation
    }
    pub const fn is_valid(self) -> bool {
        self.index != 0
    }
}

impl Default for ProviderApplicationId {
    fn default() -> Self {
        Self::from_parts(0, 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingSelection {
    pub application: Option<ProviderApplicationId>,
    pub subject: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_identity: String,
    pub service_declaration: [u8; 32],
    pub target: ProductTarget,
    pub locator: semantic_vocabulary::NativeBindingLocator,
    pub producer: NativeBindingProducer,
    pub source_occurrence_digest: [u8; 32],
    pub supplier_package: PackageKeyIdentity,
    pub source_span: source_text::SourceSpan,
}

/// One completed Build invocation and the exact binding it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildSupply {
    pub selection: NativeBindingSelection,
    pub evaluation: std::sync::Arc<build_model::receipts::NativeBuildEvaluation>,
}

impl NativeBuildSupply {
    pub fn materialization(&self) -> build_model::receipts::NativeBindingMaterialization {
        build_model::receipts::NativeBindingMaterialization {
            requirement_identity: self.selection.requirement_identity.clone(),
            service_declaration: self.selection.service_declaration,
            target: self.selection.target.canonical_name().to_owned(),
            locator: self.selection.locator.clone(),
            producer_identity: self.selection.producer.callable_identity.clone(),
            supplier_package: self.selection.supplier_package,
            source_occurrence_digest: self.selection.source_occurrence_digest,
            evaluation: std::sync::Arc::clone(&self.evaluation),
        }
    }

    pub fn materialization_for(
        &self,
        typed: &crate::source_state::TypedTrees,
        requirement: SymbolHandle,
    ) -> Result<Option<build_model::receipts::NativeBindingMaterialization>, String> {
        let Some(identity) = self
            .selection
            .requirement_identity_for(typed, requirement)?
        else {
            return Ok(None);
        };
        let mut materialization = self.materialization();
        materialization.requirement_identity = identity;
        Ok(Some(materialization))
    }
}

impl NativeBindingSelection {
    /// A binding of a generic method supplies its checked applications. An
    /// explicitly applied binding still supplies only that exact application.
    /// The original Build occurrence and evaluation remain the binding's origin.
    pub fn requirement_identity_for(
        &self,
        typed: &crate::source_state::TypedTrees,
        requirement: SymbolHandle,
    ) -> Result<Option<String>, String> {
        if requirement != self.requirement {
            if self.application.is_some() {
                return Ok(None);
            }
            let Some(closed) = typed.closed_requirement_application(requirement) else {
                return Ok(None);
            };
            if closed.template != self.requirement || closed.declaring_trait != self.subject {
                return Ok(None);
            }
        }
        require_canonical_requirement(self, typed)?;
        let (owner, authored) = typed
            .trait_machine_signature(self.requirement)
            .ok_or("native binding lost its authored requirement signature")?;
        if typed.trait_requirement_callable_identity_text(owner, authored)
            != self.requirement_identity
        {
            return Err("native binding lost its checked requirement identity".into());
        }
        if requirement != self.requirement {
            let scope =
                crate::source_state::trait_definition::AppliedTraitScope::abstract_scope(owner);
            if !typed.requirement_application_matches_contract_scope(requirement, &scope) {
                return Err("native binding application changed its declaring trait scope".into());
            }
        }
        let (actual_owner, actual) = typed
            .trait_machine_signature(requirement)
            .filter(|(actual_owner, actual)| {
                actual_owner.symbol == owner.symbol && actual.symbol == requirement
            })
            .ok_or("native binding application lost its exact requirement signature")?;
        Ok(Some(typed.trait_requirement_callable_identity_text(
            actual_owner,
            actual,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelectionIdentity {
    pub symbol: SymbolHandle,
    pub application: ProviderApplicationId,
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
    pub target: Option<ProductTarget>,
    pub subject: ProviderSelectionSubject,
    pub provider_type: ProviderSelectionIdentity,
    pub composition_mode: CompositionMode,
    pub origin: crate::declaration_selection::BuildSelectionOrigin,
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
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => identity.package,
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

pub fn application_id(
    handle: arena::Handle<crate::source_state::product_operand::ProductOperandApplication>,
) -> ProviderApplicationId {
    ProviderApplicationId::from_parts(handle.arena_index(), handle.generation())
}

pub fn source_application(
    id: ProviderApplicationId,
) -> arena::Handle<crate::source_state::product_operand::ProductOperandApplication> {
    arena::Handle::from_parts(id.arena_index(), id.generation())
}

/// Publication needs the same boundary and closed arguments that Build
/// selected. The evaluation receipt separately authenticates the locator.
pub fn require_canonical_requirement(
    selection: &NativeBindingSelection,
    typed: &crate::source_state::TypedTrees,
) -> Result<(), String> {
    let (subject, requirement) = requirement_owner(typed, selection.requirement)?;
    if subject != selection.subject {
        return Err("native binding changes its declaring boundary trait".into());
    }
    if let Some(application) = selection.application {
        let closed = typed.product_requirement_application(source_application(application))?;
        if closed.declaring_trait != subject || closed.signature.symbol != requirement {
            return Err("native binding application selects another closed requirement".into());
        }
    } else if typed.closed_requirement_application(requirement).is_some() {
        return Err("native binding lost its complete requirement application".into());
    }
    Ok(())
}

/// The boundary trait that declares `symbol` as one of its requirements. A
/// boundary with static parameters is bound only as a complete application.
pub fn requirement_owner(
    typed: &crate::source_state::TypedTrees,
    symbol: SymbolHandle,
) -> Result<(SymbolHandle, SymbolHandle), String> {
    let (owner, _) = typed
        .trait_machine_signature(symbol)
        .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == symbol)
        .ok_or("native binding requirement lost its exact boundary declaration")?;
    let declaration = typed
        .closed_requirement_application(symbol)
        .map_or(symbol, |closed| closed.template);
    if !typed
        .trait_machine_signatures(owner)
        .iter()
        .any(|signature| signature.symbol == declaration)
    {
        return Err("native binding requirement has no exact declaring requirement".into());
    }
    if !typed.trait_type_parameters(owner).is_empty() || !owner.lifetime_parameters.is_empty() {
        return Err("native binding needs its complete declaring trait application".into());
    }
    Ok((owner.symbol, symbol))
}
