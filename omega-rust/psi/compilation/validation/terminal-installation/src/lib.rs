/*
We validate source-free receiving-boundary installation contracts over Terminal
Psi artifacts (`admit_provider_installation`, `admit_component_provider`,
`admit_target_machine_selections`, `admit_target_layouts`,
`admit_private_entry_ram`, and `admit_compiler_evaluation`).
*/

/*
Our boundary-admission checks operate on canonical Terminal Psi without
reopening source trees or embedding host execution loops: provider policy,
target-machine family selection, target layouts, scalar application closures,
and synchronous call graphs are admitted alongside the artifact so both the
Terminal Psi interpreter and Omega native realization enforce identical
installation prerequisites.
*/

/*
`terminal-installation` consumes `terminal-psi` and `layout-plans` and serves
the Psi interpreter, compile-time evaluation, and Omega native realization.
*/

#![forbid(unsafe_code)]

mod evaluation_and_synchronous_calls;
mod provider_and_target_installation;
mod scalar_applications;

pub(crate) use evaluation_and_synchronous_calls::{compiler_evaluation, synchronous_calls};
pub(crate) use provider_and_target_installation::{
    component_memberships, component_provider, provider_installation, target_families,
};

pub(crate) mod entry_values {
    use semantic_vocabulary::{
        IeeeFloatValue, IntegerType, IntegerValue, ScalarType, StructuralDomainId, StructuralTypeId,
    };
    use terminal_psi::{StructuralPathSegment, TerminalAccessViewInput};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum TerminalScalarValue {
        Boolean(bool),
        Integer {
            scalar_type: IntegerType,
            value: IntegerValue,
        },
        IeeeFloat(IeeeFloatValue),
        Callable {
            contract: semantic_vocabulary::CallableContractId,
            descriptor: semantic_vocabulary::MachineDescriptorId,
        },
    }

    impl TerminalScalarValue {
        pub fn satisfies_field(self, field: &terminal_psi::StructuralFieldType) -> bool {
            if let Self::Callable { contract, .. } = self {
                return matches!(field, terminal_psi::StructuralFieldType::Scalar(ScalarType::Callable(expected)) if *expected == contract);
            }
            self.literal().is_some_and(|literal| {
                terminal_semantics::scalar_field_accepts_literal(field, &literal)
            })
        }

        /// Opaque code selections have no mathematical literal.
        pub fn literal(self) -> Option<semantic_vocabulary::ScalarTerm> {
            Some(match self {
                Self::Boolean(value) => semantic_vocabulary::ScalarTerm::Boolean(value),
                Self::Integer { scalar_type, value } => {
                    semantic_vocabulary::ScalarTerm::Integer { scalar_type, value }
                }
                Self::IeeeFloat(value) => semantic_vocabulary::ScalarTerm::IeeeFloatLiteral(value),
                Self::Callable { .. } => return None,
            })
        }

        pub const fn scalar_type(self) -> ScalarType {
            match self {
                Self::Boolean(_) => ScalarType::Boolean,
                Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
                Self::IeeeFloat(value) => ScalarType::IeeeFloat(value.format()),
                Self::Callable { contract, .. } => ScalarType::Callable(contract),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct TerminalStructuralValue {
        pub opaque_identity: u64,
        pub structural_type: StructuralTypeId,
        pub qualifications: Vec<StructuralDomainId>,
        pub projected_qualifications: Vec<terminal_psi::StructuralPathQualification>,
        pub path: Vec<StructuralPathSegment>,
    }

    impl TerminalStructuralValue {
        pub fn minted(
            opaque_identity: u64,
            structural_type: StructuralTypeId,
            qualifications: Vec<StructuralDomainId>,
        ) -> Self {
            Self {
                opaque_identity,
                structural_type,
                qualifications,
                projected_qualifications: Vec::new(),
                path: Vec::new(),
            }
        }

        pub fn absorb_qualifications(&mut self, source: &[StructuralDomainId]) {
            for domain in source {
                if !self.qualifications.contains(domain) {
                    self.qualifications.push(*domain);
                }
            }
            self.qualifications.sort();
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalAccessViewEstablishment {
        pub input: TerminalAccessViewInput,
        pub referent: TerminalStructuralValue,
    }
}

pub(crate) mod private_entry_ram {
    use semantic_vocabulary::{
        IntegerSign, IntegerValue, ScalarTerm, ScalarType, StructuralTypeId,
    };
    use terminal_psi::{StructuralFieldType, StructuralPathSegment, StructuralTypeShape};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PrivateEntryRamInitialization {
        pub position: u32,
        pub contents: Vec<u8>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PrivateEntryRamRecipe {
        pub input: terminal_psi::TerminalAccessViewInput,
        pub contents: Vec<u8>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedPrivateEntryRamRecipe {
        terminal: terminal_psi::TerminalPsiIdentity,
        layouts: crate::AdmittedTargetLayouts,
        input: terminal_psi::TerminalAccessViewInput,
        schema: StructuralTypeId,
        alignment: u64,
        correspondence: terminal_psi::AccessRamCorrespondence,
        contents: Vec<u8>,
        fields: Vec<PrivateEntryRamScalarInitialization>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PrivateEntryRamScalarInitialization {
        pub field: semantic_vocabulary::StructuralFieldId,
        pub byte_offset: usize,
        pub byte_width: usize,
        pub value: ScalarTerm,
    }

    impl CheckedPrivateEntryRamRecipe {
        pub const fn terminal(&self) -> terminal_psi::TerminalPsiIdentity {
            self.terminal
        }
        pub fn layouts(&self) -> &crate::AdmittedTargetLayouts {
            &self.layouts
        }
        pub fn target(&self) -> &str {
            self.layouts.target()
        }
        pub fn input(&self) -> &terminal_psi::TerminalAccessViewInput {
            &self.input
        }
        pub const fn schema(&self) -> StructuralTypeId {
            self.schema
        }
        pub const fn alignment(&self) -> u64 {
            self.alignment
        }
        pub fn correspondence(&self) -> &terminal_psi::AccessRamCorrespondence {
            &self.correspondence
        }
        pub fn observation_header(&self) -> Vec<u8> {
            let mut header = b"OMEGARAM".to_vec();
            header.extend(self.terminal.program_fingerprint.as_bytes());
            header.extend(self.input.placement_commitment);
            header.extend(self.input.position.to_le_bytes());
            header.extend(self.input.runtime_position.to_le_bytes());
            header.extend((self.contents.len() as u32).to_le_bytes());
            header
        }

        pub fn fields(&self) -> &[PrivateEntryRamScalarInitialization] {
            &self.fields
        }
        pub fn contents(&self) -> &[u8] {
            &self.contents
        }
    }

    pub fn private_entry_ram_identity_pair(
        module: &terminal_psi::TerminalModule,
        index: usize,
    ) -> Option<(u64, u64)> {
        let maximum = module
            .machines
            .iter()
            .flat_map(|machine| machine.structural_places.iter())
            .map(|place| place.id.get())
            .max()
            .unwrap_or(0)
            .max(1);
        let root = maximum
            .checked_add(1)?
            .checked_add(u64::try_from(index).ok()?.checked_mul(2)?)?;
        Some((root, root.checked_add(1)?))
    }

    pub fn admit_private_entry_ram(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        layouts: &crate::AdmittedTargetLayouts,
        entry: semantic_vocabulary::MachineId,
        recipe: &PrivateEntryRamRecipe,
    ) -> Result<CheckedPrivateEntryRamRecipe, String> {
        let module = artifact.module();
        if recipe.input.machine != entry
            || !module.access_view_inputs.contains(&recipe.input)
            || !matches!(
                recipe.input.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
        {
            return Err("private entry RAM recipe must answer an exact borrowed entry row".into());
        }
        let parameter = terminal_semantics::access_view_input_parameter(module, &recipe.input)
            .map_err(|reason| format!("private entry RAM row has invalid ingress: {reason:?}"))?;
        if !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        {
            return Err(
                "private entry RAM does not supply qualifications or linear custody".into(),
            );
        }
        let geometry = layouts
            .geometry(artifact)
            .map_err(|reason| format!("private entry RAM lost selected geometry: {reason:?}"))?;
        let declarations = geometry.declarations().cloned().collect::<Vec<_>>();
        let plan = module
            .access_view_plans
            .iter()
            .find(|plan| plan.carrier == parameter.structural_type)
            .ok_or("private entry RAM has no exact placed plan")?;
        let correspondence = terminal_semantics::access_ram_plan_correspondence(
            &declarations,
            &module.access_view_plans,
            plan,
            plan.schema,
        )?;
        let (byte_count, schema_alignment) =
            terminal_semantics::reference_storage_layout(declarations.iter(), plan.schema)
                .ok_or("private entry RAM schema has no complete storage")?;
        if !correspondence.required_base.admits(0) {
            return Err(
            "private entry RAM cannot meet a nonzero base congruence at its original typed root"
                .into(),
        );
        }
        let alignment = schema_alignment.max(correspondence.required_base.modulus());
        if byte_count == 0 || byte_count > 65536 || recipe.contents.len() as u64 != byte_count {
            return Err(
                "private entry RAM requires the complete nonempty bounded schema extent".into(),
            );
        }
        if module
            .structural_domains
            .iter()
            .any(|domain| domain.carrier == plan.schema)
        {
            return Err(
                "private entry RAM does not establish schema qualification predicates".into(),
            );
        }
        let schema = declarations
            .iter()
            .find(|declaration| declaration.id == plan.schema)
            .ok_or("private entry RAM schema declaration is missing")?;
        let StructuralTypeShape::Record { fields, .. } = &schema.shape else {
            return Err("private entry RAM currently constructs plain scalar records".into());
        };
        if !schema.no_code_affine_cleanup {
            return Err("private entry RAM requires checked no-code cleanup custody".into());
        }
        if schema.scalar_restriction.is_some() {
            return Err("private entry RAM does not establish declaration predicates".into());
        }
        let mut initialized = vec![false; recipe.contents.len()];
        let mut initializers = Vec::new();
        for field in fields {
            let StructuralFieldType::Scalar(
                scalar @ (ScalarType::Boolean | ScalarType::Integer(_)),
            ) = field.field_type
            else {
                return Err(
                "private entry RAM does not establish compound, atomic, erased or predicate fields"
                    .into(),
            );
            };
            if field.relevance.is_erased() {
                return Err("private entry RAM does not establish erased custody".into());
            }
            let projection = terminal_semantics::reference_storage_projection(
                declarations.iter(),
                plan.schema,
                &[StructuralPathSegment::Field(field.identity)],
            )
            .ok_or("private entry RAM field has no scalar storage projection")?;
            let width = terminal_semantics::reference_storage_byte_width(projection.stored_scalar)
                .ok_or("private entry RAM scalar has no complete codec")?
                as usize;
            let start = usize::try_from(projection.byte_offset)
                .map_err(|_| "private entry RAM field offset overflows")?;
            let end = start
                .checked_add(width)
                .ok_or("private entry RAM field extent overflows")?;
            let bytes = recipe
                .contents
                .get(start..end)
                .ok_or("private entry RAM field lies outside its allocation")?;
            let occupied = initialized
                .get_mut(start..end)
                .ok_or("private entry RAM field has invalid extent")?;
            if occupied.iter().any(|value| *value) || projection.logical_scalar != scalar {
                return Err(
                    "private entry RAM field encoding overlaps or changes its scalar carrier"
                        .into(),
                );
            }
            match scalar {
                ScalarType::Boolean if bytes != [0] && bytes != [1] => {
                    return Err("private entry RAM boolean initialization is not canonical".into());
                }
                ScalarType::Integer(integer) if integer.is_address() || integer.bits() % 8 != 0 => {
                    return Err(
                        "private entry RAM requires ordinary whole-byte integer fields".into(),
                    );
                }
                _ => {}
            }
            let value = match projection.stored_scalar {
                ScalarType::Boolean => ScalarTerm::Boolean(bytes == [1]),
                ScalarType::Integer(stored)
                    if width <= 16 && !stored.is_address() && stored.bits() % 8 == 0 =>
                {
                    let mut encoded = [0u8; 16];
                    encoded[..width].copy_from_slice(bytes);
                    let bits = u128::from_le_bytes(encoded);
                    let integer = match stored.sign() {
                        IntegerSign::Unsigned => IntegerValue::Unsigned(bits),
                        IntegerSign::Signed => IntegerValue::Signed(
                            ((bits << (128 - stored.bits())) as i128) >> (128 - stored.bits()),
                        ),
                    };
                    let ScalarType::Integer(logical) = scalar else {
                        return Err("private entry RAM codec changes scalar class".into());
                    };
                    if !logical.admits(integer) {
                        return Err(
                        "private entry RAM initializer does not inhabit its complete scalar type"
                            .into(),
                    );
                    }
                    ScalarTerm::Integer {
                        scalar_type: logical,
                        value: integer,
                    }
                }
                _ => return Err("private entry RAM does not supply this storage codec".into()),
            };
            if !terminal_semantics::scalar_field_accepts_literal(&field.field_type, &value) {
                return Err("private entry RAM initializer violates its declaration".into());
            }
            initializers.push(PrivateEntryRamScalarInitialization {
                field: field.id,
                byte_offset: start,
                byte_width: width,
                value,
            });
            occupied.fill(true);
        }
        if initialized
            .iter()
            .zip(&recipe.contents)
            .any(|(live, byte)| !live && *byte != 0)
        {
            return Err("private entry RAM padding must be initialized to canonical zero".into());
        }
        Ok(CheckedPrivateEntryRamRecipe {
            terminal: artifact.identity(),
            layouts: layouts.clone(),
            input: recipe.input.clone(),
            schema: plan.schema,
            alignment,
            correspondence,
            contents: recipe.contents.clone(),
            fields: initializers,
        })
    }

    pub fn decode_private_entry_observations(
        recipes: &[CheckedPrivateEntryRamRecipe],
        bytes: &[u8],
    ) -> Result<Vec<(u32, Vec<u8>)>, String> {
        let mut offset = 0usize;
        let mut observations = Vec::new();
        for recipe in recipes {
            let header = recipe.observation_header();
            let header_end = offset
                .checked_add(header.len())
                .ok_or("private observation header overflows")?;
            if bytes.get(offset..header_end) != Some(header.as_slice()) {
                return Err(
                "private observation lost its exact artifact, placement seal, position or extent"
                    .into(),
            );
            }
            let end = header_end
                .checked_add(recipe.contents().len())
                .ok_or("private observation extent overflows")?;
            let contents = bytes
                .get(header_end..end)
                .ok_or("private observation backing is incomplete")?;
            observations.push((recipe.input().position, contents.to_vec()));
            offset = end;
        }
        if offset != bytes.len() {
            return Err("private observation contains an extra row".into());
        }
        Ok(observations)
    }
}

pub(crate) mod target_layouts {
    use crate::AdmittedTargetMachineSelections;
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{
        LayoutApplicationId, StructuralRecordStorage, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalIdentityIndex, TerminalPsiIdentity,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TargetLayoutClosure {
        application: LayoutApplicationId,
        record: StructuralTypeId,
        policy: TerminalIdentityIndex,
    }
    impl TargetLayoutClosure {
        pub const fn application(&self) -> LayoutApplicationId {
            self.application
        }
        pub const fn record(&self) -> StructuralTypeId {
            self.record
        }
        pub const fn policy(&self) -> TerminalIdentityIndex {
            self.policy
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AdmittedTargetLayouts {
        terminal_psi: TerminalPsiIdentity,
        target: String,
        closures: Vec<TargetLayoutClosure>,
        records: Vec<StructuralTypeDeclaration>,
        lifecycles: Vec<terminal_psi::TerminalAccessLifecyclePlan>,
    }
    impl AdmittedTargetLayouts {
        pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
            self.terminal_psi
        }
        pub fn target(&self) -> &str {
            &self.target
        }
        pub fn closures(&self) -> &[TargetLayoutClosure] {
            &self.closures
        }
        pub fn resolved_records(&self) -> &[StructuralTypeDeclaration] {
            &self.records
        }
        pub fn resolved_lifecycles(&self) -> &[terminal_psi::TerminalAccessLifecyclePlan] {
            &self.lifecycles
        }
        pub fn geometry<'a>(
            &'a self,
            artifact: impl Into<terminal_codec::TerminalExecutionView<'a>>,
        ) -> Result<TargetLayoutGeometry<'a>, TargetLayoutSelectionError> {
            let artifact = artifact.into();
            if artifact.identity() != self.terminal_psi {
                return Err(TargetLayoutSelectionError::ArtifactIdentityMismatch);
            }
            Ok(TargetLayoutGeometry {
                artifact,
                layouts: self,
            })
        }
    }

    #[derive(Clone, Copy)]
    pub struct TargetLayoutGeometry<'a> {
        artifact: terminal_codec::TerminalExecutionView<'a>,
        layouts: &'a AdmittedTargetLayouts,
    }
    impl<'a> TargetLayoutGeometry<'a> {
        pub fn declaration(&self, id: StructuralTypeId) -> Option<&'a StructuralTypeDeclaration> {
            self.layouts
                .records
                .binary_search_by_key(&id, |row| row.id)
                .ok()
                .map(|position| &self.layouts.records[position])
                .or_else(|| self.artifact.module().structural_type(id))
        }
        pub fn declarations(
            &self,
        ) -> impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone + '_ {
            self.artifact
                .module()
                .structural_types
                .iter()
                .map(move |row| {
                    self.declaration(row.id)
                        .expect("canonical declaration is retained")
                })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TargetLayoutSelectionError {
        ArtifactIdentityMismatch,
        MissingMember {
            application: LayoutApplicationId,
            target: String,
        },
        StaleApplication(LayoutApplicationId),
        InvalidTargetInput(String),
        InvalidLifecycle {
            boundary: semantic_vocabulary::BoundaryMachineId,
            reason: &'static str,
        },
    }
    impl std::fmt::Display for TargetLayoutSelectionError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::InvalidLifecycle { boundary, reason } => {
                    write!(formatter, "lifecycle {boundary:?}: {reason}")
                }
                Self::InvalidTargetInput(reason) => {
                    write!(formatter, "layout target input: {reason}")
                }
                Self::ArtifactIdentityMismatch => write!(
                    formatter,
                    "layout selections belong to a different Terminal artifact"
                ),
                Self::MissingMember {
                    application,
                    target,
                } => write!(
                    formatter,
                    "layout application {} has no member for target `{target}`",
                    application.0
                ),
                Self::StaleApplication(application) => write!(
                    formatter,
                    "layout application {} has stale geometry or family custody",
                    application.0
                ),
            }
        }
    }
    impl std::error::Error for TargetLayoutSelectionError {}

    pub fn admit_target_layouts<'artifact>(
        artifact: impl Into<terminal_codec::TerminalExecutionView<'artifact>>,
        selections: &AdmittedTargetMachineSelections,
        target: &layout_plans::LayoutTargetFacts,
    ) -> Result<AdmittedTargetLayouts, TargetLayoutSelectionError> {
        let artifact = artifact.into();
        if artifact.identity() != selections.terminal_psi() {
            return Err(TargetLayoutSelectionError::ArtifactIdentityMismatch);
        }
        target
            .validate()
            .map_err(|reason| TargetLayoutSelectionError::InvalidTargetInput(reason.into()))?;
        if target.profile != selections.target() {
            return Err(TargetLayoutSelectionError::InvalidTargetInput(
                "receiving catalog and selected target disagree".into(),
            ));
        }
        let mut records = Vec::new();
        let mut closures = Vec::new();
        for application in &artifact.module().target_layout_applications {
            let stale = || TargetLayoutSelectionError::StaleApplication(application.id);
            let Some(realization) = application
                .realizations
                .binary_search_by(|row| row.target.as_str().cmp(selections.target()))
                .ok()
                .map(|position| &application.realizations[position])
            else {
                continue;
            };
            if realization.pointer_size != target.pointer_size
                || realization.pointer_alignment != target.pointer_alignment
                || realization.case_tag_size != target.case_tag_size
                || realization.case_tag_alignment != target.case_tag_alignment
                || realization.little_endian != target.little_endian
            {
                return Err(stale());
            }
            let mut record = artifact
                .module()
                .structural_type(application.record)
                .ok_or_else(stale)?
                .clone();
            let StructuralTypeShape::Record { fields, storage } = &mut record.shape else {
                return Err(stale());
            };
            if *storage != StructuralRecordStorage::TargetLayout(application.id) {
                return Err(stale());
            }
            *storage = StructuralRecordStorage::Authored(realization.storage);
            for field in fields
                .iter_mut()
                .filter(|field| !field.relevance.is_erased())
            {
                let row = realization
                    .fields
                    .binary_search_by_key(&field.id, |row| row.field)
                    .ok()
                    .map(|position| &realization.fields[position])
                    .ok_or_else(stale)?;
                field.storage = Some(row.storage.clone());
            }
            closures.push(TargetLayoutClosure {
                application: application.id,
                record: application.record,
                policy: application.policy_identity,
            });
            records.push(record);
        }
        records.sort_unstable_by_key(|record| record.id);
        let mut lifecycles = Vec::new();
        for plan in &artifact.module().access_lifecycle_plans {
            if let Some(input) = &plan.target
                && !lifecycle_target_matches(input, target).map_err(|reason| {
                    TargetLayoutSelectionError::InvalidLifecycle {
                        boundary: plan.boundary,
                        reason,
                    }
                })?
            {
                continue;
            }
            if lifecycles.last().is_some_and(
                |previous: &terminal_psi::TerminalAccessLifecyclePlan| {
                    previous.boundary == plan.boundary
                },
            ) {
                return Err(TargetLayoutSelectionError::InvalidLifecycle {
                    boundary: plan.boundary,
                    reason: "multiple placements apply to the selected target",
                });
            }
            lifecycles.push(plan.clone());
        }
        Ok(AdmittedTargetLayouts {
            terminal_psi: artifact.identity(),
            target: selections.target().to_owned(),
            closures,
            records,
            lifecycles,
        })
    }

    fn lifecycle_target_matches(
        input: &terminal_psi::TargetLayoutInput,
        target: &layout_plans::LayoutTargetFacts,
    ) -> Result<bool, &'static str> {
        if input.target != target.profile {
            return Ok(false);
        }
        if input.pointer_size != target.pointer_size
            || input.pointer_alignment != target.pointer_alignment
        {
            return Err("pointer geometry differs from the receiving target");
        }
        if input.case_tag_size != target.case_tag_size
            || input.case_tag_alignment != target.case_tag_alignment
        {
            return Err("case tag geometry differs from the receiving target");
        }
        if input.little_endian != target.little_endian {
            return Err("byte order differs from the receiving target");
        }
        Ok(true)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn lifecycle_admission_checks_the_selected_catalog_not_just_its_profile_name() {
            let target = layout_plans::LayoutTargetFacts {
                profile: "linux_x86_64".into(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            };
            let valid = terminal_psi::TargetLayoutInput {
                target: target.profile.clone(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            };
            assert_eq!(lifecycle_target_matches(&valid, &target), Ok(true));
            let mut other = valid.clone();
            other.target = "linux_arm64".into();
            assert_eq!(lifecycle_target_matches(&other, &target), Ok(false));
            let mut altered = valid.clone();
            altered.pointer_size = 4;
            assert_eq!(
                lifecycle_target_matches(&altered, &target),
                Err("pointer geometry differs from the receiving target")
            );
            let mut altered = valid.clone();
            altered.pointer_alignment = 4;
            assert!(lifecycle_target_matches(&altered, &target).is_err());
            let mut altered = valid.clone();
            altered.case_tag_size = 8;
            assert_eq!(
                lifecycle_target_matches(&altered, &target),
                Err("case tag geometry differs from the receiving target")
            );
            let mut altered = valid.clone();
            altered.case_tag_alignment = 2;
            assert!(lifecycle_target_matches(&altered, &target).is_err());
            let mut altered = valid;
            altered.little_endian = false;
            assert_eq!(
                lifecycle_target_matches(&altered, &target),
                Err("byte order differs from the receiving target")
            );
        }
    }
}

pub use compiler_evaluation::{AdmittedCompilerEvaluation, admit_compiler_evaluation};
pub use component_provider::{AdmittedComponentProvider, admit_component_provider};
pub use entry_values::{
    TerminalAccessViewEstablishment, TerminalScalarValue, TerminalStructuralValue,
};
pub use layout_plans::LayoutTargetFacts;
pub use private_entry_ram::{
    CheckedPrivateEntryRamRecipe, PrivateEntryRamInitialization, PrivateEntryRamRecipe,
    PrivateEntryRamScalarInitialization, admit_private_entry_ram,
    decode_private_entry_observations, private_entry_ram_identity_pair,
};
pub use provider_installation::{
    AdmittedAssumedBinding, AdmittedProviderSelections, AssumedBindingRejection,
    AssumedBindingSelection, ProviderInstallationError, ProviderInstallationSelection,
    admit_provider_execution_installation, admit_provider_installation,
    admit_provider_installation_from_artifact,
    admit_provider_installation_with_assumed_bindings_from_artifact,
};
pub use scalar_applications::{
    AdmittedScalarApplications, AdmittedScalarEntry, CallingAbiStorageDemand, ScalarAddressModel,
    ScalarApplicationClosure, ScalarApplicationObservations, ScalarApplicationValues,
    ScalarConditionSelection, ScalarEntryChecks, ScalarEntryPrerequisites,
    ScalarLayoutObservationError, ScalarScopeInputs,
};
pub use synchronous_calls::{SynchronousCallClosure, SynchronousCallGraph};
pub use target_families::{
    AdmittedTargetMachineSelections, TargetMachineSelection, TargetMachineSelectionError,
    admit_target_machine_selections, admitted_boundary_member, resolve_target_machine_member,
};
pub use target_layouts::{
    AdmittedTargetLayouts, TargetLayoutClosure, TargetLayoutGeometry, TargetLayoutSelectionError,
    admit_target_layouts,
};
