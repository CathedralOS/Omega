//! Checked initialization demand for freshly constructed invocation-owned RAM.
//!
//! A recipe never describes a surviving external object, host pointer or MMIO.
//! Admission checks the exact entry row and complete typed contents. Consumers
//! must construct their original allocation from these contents before lending
//! it, prove the concrete alignment, and retain it until the loan retires. This
//! receipt supplies no authority to access another allocation or call a host.

use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarTerm, ScalarType, StructuralTypeId};
use terminal_psi::{StructuralFieldType, StructuralPathSegment, StructuralTypeShape};

/// Explicit sponsor request to construct fresh typed RAM at an authored position.
/// The selected canonical entry supplies the complete row before admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateEntryRamInitialization {
    pub position: u32,
    pub contents: Vec<u8>,
}

/// Untrusted request to construct one fresh private backing for an exact row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateEntryRamRecipe {
    pub input: terminal_psi::TerminalPlacedViewInput,
    /// Complete initialized storage in canonical little-endian scalar encoding.
    pub contents: Vec<u8>,
}

/// A demand checked against one immutable canonical Terminal artifact.
/// Physical allocation and invocation custody still belong to the consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPrivateEntryRamRecipe {
    terminal: terminal_psi::TerminalPsiIdentity,
    layouts: crate::AdmittedTargetLayouts,
    input: terminal_psi::TerminalPlacedViewInput,
    schema: StructuralTypeId,
    alignment: u64,
    correspondence: terminal_psi::PlacedRamCorrespondence,
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
    /// Exact target-dependent selected geometry retained from admission.
    pub fn layouts(&self) -> &crate::AdmittedTargetLayouts {
        &self.layouts
    }
    pub fn target(&self) -> &str {
        self.layouts.target()
    }
    pub fn input(&self) -> &terminal_psi::TerminalPlacedViewInput {
        &self.input
    }
    pub const fn schema(&self) -> StructuralTypeId {
        self.schema
    }
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
    pub fn correspondence(&self) -> &terminal_psi::PlacedRamCorrespondence {
        &self.correspondence
    }
    /// Authenticated by the private runtime channel and independently replayed
    /// post-return writer, with exact canonical artifact, row seal and extent.
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

/// Choose fresh symbolic storage identities outside every declared local home.
/// These names carry no allocation, access or host authority. The constructor
/// still creates and retains the actual original storage before any view loan.
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

/// Check a recipe against selected record geometry and exact canonical ingress.
/// An empty recipe roster remains absent supply. Allocation identity is minted
/// by the runtime consumer, rather than encoded in this initialization request.
pub fn admit_private_entry_ram(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    layouts: &crate::AdmittedTargetLayouts,
    entry: semantic_vocabulary::MachineId,
    recipe: &PrivateEntryRamRecipe,
) -> Result<CheckedPrivateEntryRamRecipe, String> {
    let module = artifact.module();
    if recipe.input.machine != entry
        || !module.placed_view_inputs.contains(&recipe.input)
        || !matches!(
            recipe.input.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
    {
        return Err("private entry RAM recipe must answer an exact borrowed entry row".into());
    }
    let parameter = terminal_semantics::placed_view_input_parameter(module, &recipe.input)
        .map_err(|reason| format!("private entry RAM row has invalid ingress: {reason:?}"))?;
    if !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
    {
        return Err("private entry RAM does not supply qualifications or linear custody".into());
    }
    let geometry = layouts
        .geometry(artifact)
        .map_err(|reason| format!("private entry RAM lost selected geometry: {reason:?}"))?;
    let declarations = geometry.declarations().cloned().collect::<Vec<_>>();
    let plan = module
        .placed_view_plans
        .iter()
        .find(|plan| plan.carrier == parameter.structural_type)
        .ok_or("private entry RAM has no exact placed plan")?;
    let correspondence = terminal_semantics::placed_ram_plan_correspondence(
        &declarations,
        &module.placed_view_plans,
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
        return Err("private entry RAM does not establish schema qualification predicates".into());
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
        let StructuralFieldType::Scalar(scalar @ (ScalarType::Boolean | ScalarType::Integer(_))) =
            field.field_type
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
            .ok_or("private entry RAM scalar has no complete codec")? as usize;
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
                "private entry RAM field encoding overlaps or changes its scalar carrier".into(),
            );
        }
        match scalar {
            ScalarType::Boolean if bytes != [0] && bytes != [1] => {
                return Err("private entry RAM boolean initialization is not canonical".into());
            }
            ScalarType::Integer(integer) if integer.is_address() || integer.bits() % 8 != 0 => {
                return Err("private entry RAM requires ordinary whole-byte integer fields".into());
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
