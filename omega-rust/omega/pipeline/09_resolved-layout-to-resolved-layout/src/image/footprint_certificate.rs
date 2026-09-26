//! Full replay of footprint coverage, inventory and text derivation into one
//! certificate - reachable today only from tests, and pinned as source text by
//! tests/architecture/native_image_identity.rs.

use crate::image::{
    CompilerEntryFootprintBindingEvidence, CompilerFunctionValidationEvidence,
    CompilerTextValidationEvidence, PlacedExecutableRegionInventory,
};
#[cfg(test)]
use crate::image::{
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest, EncodedCompilerTextDigest,
    FinalCompilerTextDigest,
};
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};

pub const FINAL_FOOTPRINT_CERTIFICATE_MARKER: &str = "omega.final-footprint-certificate.current";

macro_rules! footprint_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

footprint_digest!(FinalFootprintCoverageDigest);
footprint_digest!(FinalFootprintPlacementBindingDigest);
footprint_digest!(FinalFootprintCertificateDigest);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FinalFootprintClass {
    CompilerFunctions,
    CompilerFunctionRelocationEnvelope,
    CompilerFunctionCallReturnMechanics,
    CompilerFunctionInstructionEnumeration,
    CompilerFunctionBodySpecification,
    CatalogCheckedAssembly,
    ImportThunks,
    RelaxationProducts,
    Veneers,
    GeneratedStubs,
    AdmittedLeaves,
}

impl FinalFootprintClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::CompilerFunctions => "compiler_functions",
            Self::ImportThunks => "import_thunks",
            Self::RelaxationProducts => "relaxation_products",
            Self::Veneers => "veneers",
            Self::GeneratedStubs => "generated_stubs",
            Self::CompilerFunctionRelocationEnvelope => "compiler_function_relocation_envelope",
            Self::CompilerFunctionCallReturnMechanics => "compiler_function_call_return_mechanics",
            Self::CompilerFunctionInstructionEnumeration => {
                "compiler_function_instruction_enumeration"
            }
            Self::CompilerFunctionBodySpecification => "compiler_function_body_specification",
            Self::CatalogCheckedAssembly => "catalog_checked_assembly",
            Self::AdmittedLeaves => "admitted_leaves",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::CompilerFunctions => 1,
            Self::CompilerFunctionRelocationEnvelope => 2,
            Self::CompilerFunctionCallReturnMechanics => 3,
            Self::CompilerFunctionInstructionEnumeration => 11,
            Self::CompilerFunctionBodySpecification => 12,
            Self::CatalogCheckedAssembly => 4,
            Self::ImportThunks => 5,
            Self::RelaxationProducts => 6,
            Self::Veneers => 7,
            Self::GeneratedStubs => 8,
            Self::AdmittedLeaves => 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalFootprintCoverage {
    pub enumeration_complete: bool,
    pub region_enumeration_complete: bool,
    pub footprint_enumeration_complete: bool,
    pub covered_classes: Vec<FinalFootprintClass>,
    pub absent_by_construction_classes: Vec<FinalFootprintClass>,
    pub final_byte_validated_classes: Vec<FinalFootprintClass>,
    pub missing_classes: Vec<FinalFootprintClass>,
}

impl FinalFootprintCoverage {
    pub fn current() -> Self {
        Self {
            enumeration_complete: true,
            region_enumeration_complete: true,
            footprint_enumeration_complete: true,
            covered_classes: vec![
                FinalFootprintClass::CompilerFunctions,
                FinalFootprintClass::ImportThunks,
            ],
            absent_by_construction_classes: vec![
                FinalFootprintClass::RelaxationProducts,
                FinalFootprintClass::Veneers,
                FinalFootprintClass::GeneratedStubs,
                FinalFootprintClass::AdmittedLeaves,
            ],
            final_byte_validated_classes: vec![
                FinalFootprintClass::CompilerFunctionRelocationEnvelope,
                FinalFootprintClass::CompilerFunctionCallReturnMechanics,
                FinalFootprintClass::CompilerFunctionInstructionEnumeration,
                FinalFootprintClass::CompilerFunctionBodySpecification,
                FinalFootprintClass::CatalogCheckedAssembly,
                FinalFootprintClass::ImportThunks,
            ],
            missing_classes: Vec::new(),
        }
    }

    pub fn validate_normalized(&self) -> Result<(), Diagnostic> {
        for (name, classes) in [
            ("covered", &self.covered_classes),
            (
                "absent-by-construction",
                &self.absent_by_construction_classes,
            ),
            ("final-byte-validated", &self.final_byte_validated_classes),
            ("missing", &self.missing_classes),
        ] {
            if classes.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(Diagnostic::error(format!(
                    "final footprint certificate {name} classes are not strictly normalized"
                )));
            }
        }
        for (left_name, left, right_name, right) in [
            (
                "covered",
                &self.covered_classes,
                "absent-by-construction",
                &self.absent_by_construction_classes,
            ),
            (
                "covered",
                &self.covered_classes,
                "missing",
                &self.missing_classes,
            ),
            (
                "absent-by-construction",
                &self.absent_by_construction_classes,
                "missing",
                &self.missing_classes,
            ),
        ] {
            if let Some(class) = left.iter().find(|class| right.contains(class)) {
                return Err(Diagnostic::error(format!(
                    "final footprint class `{}` is both {left_name} and {right_name}",
                    class.name()
                )));
            }
        }
        if self.enumeration_complete
            != (self.region_enumeration_complete && self.footprint_enumeration_complete)
        {
            return Err(Diagnostic::error(
                "final footprint certificate completeness flags disagree",
            ));
        }
        if self.footprint_enumeration_complete && !self.missing_classes.is_empty() {
            return Err(Diagnostic::error(
                "complete final footprint enumeration cannot retain missing classes",
            ));
        }
        Ok(())
    }

    pub fn report_fingerprint(&self) -> u64 {
        let mut hash = FNV_OFFSET;
        fingerprint_bytes(
            &mut hash,
            &[
                u8::from(self.enumeration_complete),
                u8::from(self.region_enumeration_complete),
                u8::from(self.footprint_enumeration_complete),
            ],
        );
        for classes in [
            &self.covered_classes,
            &self.absent_by_construction_classes,
            &self.final_byte_validated_classes,
            &self.missing_classes,
        ] {
            fingerprint_bytes(&mut hash, &(classes.len() as u64).to_le_bytes());
            for class in classes {
                fingerprint_bytes(&mut hash, &[class.tag()]);
            }
        }
        hash
    }

    pub fn digest(&self) -> FinalFootprintCoverageDigest {
        let mut digest = Sha256::new();
        digest.update(b"omega.final-footprint-coverage.sha256.v1\0");
        digest.update([
            u8::from(self.enumeration_complete),
            u8::from(self.region_enumeration_complete),
            u8::from(self.footprint_enumeration_complete),
        ]);
        for classes in [
            &self.covered_classes,
            &self.absent_by_construction_classes,
            &self.final_byte_validated_classes,
            &self.missing_classes,
        ] {
            digest.update((classes.len() as u64).to_le_bytes());
            for class in classes {
                digest.update([class.tag()]);
            }
        }
        FinalFootprintCoverageDigest::from_digest(digest.finalize().into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalFootprintCertificate {
    pub marker: &'static str,
    pub certificate_digest: FinalFootprintCertificateDigest,
    /// Compact report compatibility only.
    pub certificate_report_fingerprint: u64,
    pub coverage_digest: FinalFootprintCoverageDigest,
    /// Compact report compatibility only.
    pub coverage_report_fingerprint: u64,
    pub coverage: FinalFootprintCoverage,
    /// Compact report coordinate beside exact retained state footprints.
    pub boundary_contract_report_fingerprint: Option<u64>,
    /// Compact report compatibility only; exact footprint and inventory
    /// commitments carry authority.
    pub implementation_evidence_report_fingerprint: u64,
    pub implementation_fragment_count: usize,
    /// Compact report coordinate for callback rows structurally replayed before
    /// image emission.
    pub callback_placement_identity_report_fingerprint: u64,
    pub compiler_text_validation: CompilerTextValidationEvidence,
    pub compiler_function_validation: CompilerFunctionValidationEvidence,
    pub compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
    pub inventory: PlacedExecutableRegionInventory,
    pub boundary_placement_binding_digest: FinalFootprintPlacementBindingDigest,
    /// Compact report compatibility only.
    pub boundary_placement_binding_report_fingerprint: u64,
}

impl FinalFootprintCertificate {
    pub fn current(
        boundary_contract_report_fingerprint: Option<u64>,
        implementation_evidence_report_fingerprint: u64,
        implementation_fragment_count: usize,
        callback_placement_identity_report_fingerprint: u64,
        compiler_text_validation: CompilerTextValidationEvidence,
        compiler_function_validation: CompilerFunctionValidationEvidence,
        compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
        inventory: PlacedExecutableRegionInventory,
    ) -> Result<Self, Diagnostic> {
        if !compiler_text_validation.has_valid_derivation_digest() {
            return Err(Diagnostic::error(
                "compiler text validation evidence has an invalid strong derivation digest",
            ));
        }
        crate::image::final_image::validate_placed_executable_region_inventory_digest(&inventory)?;
        if !inventory.unclassified_gaps.is_empty() {
            return Err(Diagnostic::error(
                "region-complete final footprint certificate cannot retain executable gaps",
            ));
        }
        if compiler_function_validation
            .boundary_contract_report_fingerprint
            .is_some()
            && boundary_contract_report_fingerprint
                != compiler_function_validation.boundary_contract_report_fingerprint
        {
            return Err(Diagnostic::error(
                "final compiler-function report names a different boundary contract",
            ));
        }
        validate_entry_footprint_binding(
            boundary_contract_report_fingerprint,
            implementation_evidence_report_fingerprint,
            compiler_function_validation,
            compiler_entry_footprint_binding,
            &inventory,
        )?;
        let coverage = FinalFootprintCoverage::current();
        coverage.validate_normalized()?;
        let coverage_digest = coverage.digest();
        let coverage_report_fingerprint = coverage.report_fingerprint();
        let boundary_placement_binding_digest = placement_binding_digest(
            boundary_contract_report_fingerprint,
            implementation_evidence_report_fingerprint,
            implementation_fragment_count,
            callback_placement_identity_report_fingerprint,
            &compiler_text_validation,
            compiler_function_validation,
            compiler_entry_footprint_binding,
            &inventory,
        );
        let boundary_placement_binding_report_fingerprint = placement_binding_report_fingerprint(
            boundary_contract_report_fingerprint,
            implementation_evidence_report_fingerprint,
            callback_placement_identity_report_fingerprint,
            compiler_text_validation.derivation_report_fingerprint,
            compiler_function_validation.evidence_report_fingerprint(),
            compiler_entry_footprint_binding
                .map(|binding| binding.evidence_report_fingerprint)
                .unwrap_or_default(),
            inventory.inventory_report_fingerprint,
        );
        let certificate_report_fingerprint = certificate_report_fingerprint(
            coverage_report_fingerprint,
            boundary_placement_binding_report_fingerprint,
            compiler_text_validation.derivation_report_fingerprint,
            compiler_function_validation.evidence_report_fingerprint(),
            compiler_entry_footprint_binding
                .map(|binding| binding.evidence_report_fingerprint)
                .unwrap_or_default(),
            inventory.inventory_report_fingerprint,
        );
        let certificate_digest = certificate_digest(
            coverage_digest,
            boundary_placement_binding_digest,
            &compiler_text_validation,
            compiler_function_validation,
            compiler_entry_footprint_binding,
            &inventory,
        );
        Ok(Self {
            marker: FINAL_FOOTPRINT_CERTIFICATE_MARKER,
            certificate_digest,
            certificate_report_fingerprint,
            coverage_digest,
            coverage_report_fingerprint,
            coverage,
            boundary_contract_report_fingerprint,
            implementation_evidence_report_fingerprint,
            implementation_fragment_count,
            callback_placement_identity_report_fingerprint,
            compiler_text_validation,
            compiler_function_validation,
            compiler_entry_footprint_binding,
            inventory,
            boundary_placement_binding_digest,
            boundary_placement_binding_report_fingerprint,
        })
    }

    pub fn validate_identity(&self) -> Result<(), Diagnostic> {
        if self.marker != FINAL_FOOTPRINT_CERTIFICATE_MARKER {
            return Err(Diagnostic::error(
                "unsupported final footprint certificate marker",
            ));
        }
        self.coverage.validate_normalized()?;
        crate::image::final_image::validate_placed_executable_region_inventory_digest(
            &self.inventory,
        )?;
        if !self.compiler_text_validation.has_valid_derivation_digest() {
            return Err(Diagnostic::error(
                "compiler text validation evidence has an invalid strong derivation digest",
            ));
        }
        if self
            .compiler_function_validation
            .boundary_contract_report_fingerprint
            .is_some()
            && self.boundary_contract_report_fingerprint
                != self
                    .compiler_function_validation
                    .boundary_contract_report_fingerprint
        {
            return Err(Diagnostic::error(
                "final compiler-function report names a different boundary contract",
            ));
        }
        if self.coverage.region_enumeration_complete && !self.inventory.unclassified_gaps.is_empty()
        {
            return Err(Diagnostic::error(
                "region-complete final footprint certificate retains executable gaps",
            ));
        }
        validate_entry_footprint_binding(
            self.boundary_contract_report_fingerprint,
            self.implementation_evidence_report_fingerprint,
            self.compiler_function_validation,
            self.compiler_entry_footprint_binding,
            &self.inventory,
        )?;
        let expected_coverage = self.coverage.report_fingerprint();
        if self.coverage_report_fingerprint != expected_coverage {
            return Err(Diagnostic::error(
                "final footprint certificate coverage fingerprint mismatch",
            ));
        }
        let expected_coverage_digest = self.coverage.digest();
        if self.coverage_digest != expected_coverage_digest {
            return Err(Diagnostic::error(
                "final footprint certificate coverage digest mismatch",
            ));
        }
        let expected_binding_digest = placement_binding_digest(
            self.boundary_contract_report_fingerprint,
            self.implementation_evidence_report_fingerprint,
            self.implementation_fragment_count,
            self.callback_placement_identity_report_fingerprint,
            &self.compiler_text_validation,
            self.compiler_function_validation,
            self.compiler_entry_footprint_binding,
            &self.inventory,
        );
        if self.boundary_placement_binding_digest != expected_binding_digest {
            return Err(Diagnostic::error(
                "final footprint certificate strong placement binding mismatch",
            ));
        }
        let expected_binding = placement_binding_report_fingerprint(
            self.boundary_contract_report_fingerprint,
            self.implementation_evidence_report_fingerprint,
            self.callback_placement_identity_report_fingerprint,
            self.compiler_text_validation.derivation_report_fingerprint,
            self.compiler_function_validation
                .evidence_report_fingerprint(),
            self.compiler_entry_footprint_binding
                .map(|binding| binding.evidence_report_fingerprint)
                .unwrap_or_default(),
            self.inventory.inventory_report_fingerprint,
        );
        if self.boundary_placement_binding_report_fingerprint != expected_binding {
            return Err(Diagnostic::error(
                "final footprint certificate placement binding mismatch",
            ));
        }
        let expected_certificate = certificate_report_fingerprint(
            expected_coverage,
            expected_binding,
            self.compiler_text_validation.derivation_report_fingerprint,
            self.compiler_function_validation
                .evidence_report_fingerprint(),
            self.compiler_entry_footprint_binding
                .map(|binding| binding.evidence_report_fingerprint)
                .unwrap_or_default(),
            self.inventory.inventory_report_fingerprint,
        );
        if self.certificate_report_fingerprint != expected_certificate {
            return Err(Diagnostic::error(
                "final footprint certificate identity mismatch",
            ));
        }
        let expected_certificate_digest = certificate_digest(
            expected_coverage_digest,
            expected_binding_digest,
            &self.compiler_text_validation,
            self.compiler_function_validation,
            self.compiler_entry_footprint_binding,
            &self.inventory,
        );
        if self.certificate_digest != expected_certificate_digest {
            return Err(Diagnostic::error(
                "final footprint certificate strong identity mismatch",
            ));
        }
        Ok(())
    }
}

fn validate_entry_footprint_binding(
    boundary_contract_report_fingerprint: Option<u64>,
    implementation_evidence_report_fingerprint: u64,
    compiler_function_validation: CompilerFunctionValidationEvidence,
    binding: Option<CompilerEntryFootprintBindingEvidence>,
    inventory: &PlacedExecutableRegionInventory,
) -> Result<(), Diagnostic> {
    match (boundary_contract_report_fingerprint, binding) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(Diagnostic::error(
            "final footprint certificate retains entry-footprint custody without a boundary contract",
        )),
        (Some(_), None) => Err(Diagnostic::error(
            "final footprint certificate lacks exact entry-footprint mutation custody",
        )),
        (Some(_), Some(binding)) => {
            if !binding.validate_identity()
                || binding.footprint_report_fingerprint
                    != implementation_evidence_report_fingerprint
                || binding.final_region_binding_report_fingerprint
                    != compiler_function_validation.final_region_binding_report_fingerprint
                || binding.resulting_inventory_report_fingerprint
                    != inventory.inventory_report_fingerprint
                || binding.resulting_inventory_digest != inventory.inventory_digest
                || binding.prior_inventory_report_fingerprint
                    == binding.resulting_inventory_report_fingerprint
                || binding.prior_inventory_digest == binding.resulting_inventory_digest
            {
                return Err(Diagnostic::error(
                    "final footprint certificate entry-footprint mutation custody drifted",
                ));
            }
            Ok(())
        }
    }
}

fn placement_binding_digest(
    boundary_contract_report_fingerprint: Option<u64>,
    implementation_evidence_report_fingerprint: u64,
    implementation_fragment_count: usize,
    callback_placement_identity_report_fingerprint: u64,
    compiler_text_validation: &CompilerTextValidationEvidence,
    compiler_function_validation: CompilerFunctionValidationEvidence,
    compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
    inventory: &PlacedExecutableRegionInventory,
) -> FinalFootprintPlacementBindingDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.final-footprint-placement-binding.sha256.v1\0");
    digest.update([u8::from(boundary_contract_report_fingerprint.is_some())]);
    digest.update(
        boundary_contract_report_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    digest.update(implementation_evidence_report_fingerprint.to_le_bytes());
    digest.update((implementation_fragment_count as u64).to_le_bytes());
    digest.update(callback_placement_identity_report_fingerprint.to_le_bytes());
    digest.update(compiler_text_validation.derivation_digest.as_bytes());
    update_compiler_function_validation_digest(&mut digest, compiler_function_validation);
    match compiler_entry_footprint_binding {
        Some(binding) => {
            digest.update([1]);
            digest.update(binding.evidence_digest.as_bytes());
        }
        None => digest.update([0]),
    }
    digest.update(inventory.inventory_digest.as_bytes());
    FinalFootprintPlacementBindingDigest::from_digest(digest.finalize().into())
}

fn certificate_digest(
    coverage_digest: FinalFootprintCoverageDigest,
    boundary_placement_binding_digest: FinalFootprintPlacementBindingDigest,
    compiler_text_validation: &CompilerTextValidationEvidence,
    compiler_function_validation: CompilerFunctionValidationEvidence,
    compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
    inventory: &PlacedExecutableRegionInventory,
) -> FinalFootprintCertificateDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.final-footprint-certificate.sha256.v1\0");
    digest.update((FINAL_FOOTPRINT_CERTIFICATE_MARKER.len() as u64).to_le_bytes());
    digest.update(FINAL_FOOTPRINT_CERTIFICATE_MARKER.as_bytes());
    digest.update(coverage_digest.as_bytes());
    digest.update(boundary_placement_binding_digest.as_bytes());
    digest.update(compiler_text_validation.derivation_digest.as_bytes());
    update_compiler_function_validation_digest(&mut digest, compiler_function_validation);
    match compiler_entry_footprint_binding {
        Some(binding) => {
            digest.update([1]);
            digest.update(binding.evidence_digest.as_bytes());
        }
        None => digest.update([0]),
    }
    digest.update(inventory.inventory_digest.as_bytes());
    FinalFootprintCertificateDigest::from_digest(digest.finalize().into())
}

fn update_compiler_function_validation_digest(
    digest: &mut Sha256,
    evidence: CompilerFunctionValidationEvidence,
) {
    digest.update(b"omega.compiler-function-validation-structure.v1\0");
    digest.update(evidence.evidence_digest().as_bytes());
}

fn placement_binding_report_fingerprint(
    boundary_contract_report_fingerprint: Option<u64>,
    implementation_evidence_report_fingerprint: u64,
    callback_placement_identity_report_fingerprint: u64,
    compiler_text_derivation_report_fingerprint: u64,
    compiler_function_validation_report_fingerprint: u64,
    compiler_entry_footprint_binding_report_fingerprint: u64,
    inventory_report_fingerprint: u64,
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(
        &mut hash,
        &[
            u8::from(boundary_contract_report_fingerprint.is_some()),
            0x42,
            0x50,
            0x42,
        ],
    );
    fingerprint_bytes(
        &mut hash,
        &boundary_contract_report_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &implementation_evidence_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &callback_placement_identity_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_text_derivation_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_function_validation_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_entry_footprint_binding_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(&mut hash, &inventory_report_fingerprint.to_le_bytes());
    hash
}

fn certificate_report_fingerprint(
    coverage_report_fingerprint: u64,
    boundary_placement_binding_report_fingerprint: u64,
    compiler_text_derivation_report_fingerprint: u64,
    compiler_function_validation_report_fingerprint: u64,
    compiler_entry_footprint_binding_report_fingerprint: u64,
    inventory_report_fingerprint: u64,
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(&mut hash, FINAL_FOOTPRINT_CERTIFICATE_MARKER.as_bytes());
    fingerprint_bytes(&mut hash, &coverage_report_fingerprint.to_le_bytes());
    fingerprint_bytes(
        &mut hash,
        &boundary_placement_binding_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_text_derivation_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_function_validation_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_entry_footprint_binding_report_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(&mut hash, &inventory_report_fingerprint.to_le_bytes());
    hash
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[cfg(test)]
mod tests;
