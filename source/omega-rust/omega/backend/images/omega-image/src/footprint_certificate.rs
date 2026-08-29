use crate::{
    CompilerEntryFootprintBindingEvidence, CompilerFunctionValidationEvidence,
    CompilerTextValidationEvidence, PlacedExecutableRegionInventory,
};
#[cfg(test)]
use crate::{
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest, EncodedCompilerTextDigest,
    FinalCompilerTextDigest,
};
use psi_diagnostics::Diagnostic;

pub const FINAL_FOOTPRINT_CERTIFICATE_MARKER: &str = "omega.final-footprint-certificate.current";

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

    pub fn fingerprint(&self) -> u64 {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalFootprintCertificate {
    pub marker: &'static str,
    pub certificate_fingerprint: u64,
    pub coverage_fingerprint: u64,
    pub coverage: FinalFootprintCoverage,
    pub boundary_contract_fingerprint: Option<u64>,
    pub implementation_evidence_fingerprint: u64,
    pub implementation_fragment_count: usize,
    pub callback_placement_identity_fingerprint: u64,
    pub compiler_text_validation: CompilerTextValidationEvidence,
    pub compiler_function_validation: CompilerFunctionValidationEvidence,
    pub compiler_entry_footprint_binding: Option<CompilerEntryFootprintBindingEvidence>,
    pub inventory: PlacedExecutableRegionInventory,
    pub boundary_placement_binding_fingerprint: u64,
}

impl FinalFootprintCertificate {
    pub fn current(
        boundary_contract_fingerprint: Option<u64>,
        implementation_evidence_fingerprint: u64,
        implementation_fragment_count: usize,
        callback_placement_identity_fingerprint: u64,
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
        if !inventory.unclassified_gaps.is_empty() {
            return Err(Diagnostic::error(
                "region-complete final footprint certificate cannot retain executable gaps",
            ));
        }
        if compiler_function_validation.body_specification_instruction_count > 0
            && boundary_contract_fingerprint
                != Some(
                    compiler_function_validation.body_specification_boundary_contract_fingerprint,
                )
        {
            return Err(Diagnostic::error(
                "final body-specification footprint evidence names a different boundary contract",
            ));
        }
        if compiler_function_validation.fixed_mechanics_instruction_count > 0
            && boundary_contract_fingerprint
                != Some(compiler_function_validation.fixed_mechanics_boundary_contract_fingerprint)
        {
            return Err(Diagnostic::error(
                "final call-return footprint evidence names a different boundary contract",
            ));
        }
        validate_entry_footprint_binding(
            boundary_contract_fingerprint,
            implementation_evidence_fingerprint,
            compiler_function_validation,
            compiler_entry_footprint_binding,
            &inventory,
        )?;
        let coverage = FinalFootprintCoverage::current();
        coverage.validate_normalized()?;
        let coverage_fingerprint = coverage.fingerprint();
        let boundary_placement_binding_fingerprint = placement_binding_fingerprint(
            boundary_contract_fingerprint,
            implementation_evidence_fingerprint,
            callback_placement_identity_fingerprint,
            compiler_text_validation.derivation_fingerprint,
            compiler_function_validation.evidence_fingerprint(),
            compiler_entry_footprint_binding
                .map(|binding| binding.evidence_fingerprint)
                .unwrap_or_default(),
            inventory.inventory_fingerprint,
        );
        let certificate_fingerprint = certificate_fingerprint(
            coverage_fingerprint,
            boundary_placement_binding_fingerprint,
            compiler_text_validation.derivation_fingerprint,
            compiler_function_validation.evidence_fingerprint(),
            compiler_entry_footprint_binding
                .map(|binding| binding.evidence_fingerprint)
                .unwrap_or_default(),
            inventory.inventory_fingerprint,
        );
        Ok(Self {
            marker: FINAL_FOOTPRINT_CERTIFICATE_MARKER,
            certificate_fingerprint,
            coverage_fingerprint,
            coverage,
            boundary_contract_fingerprint,
            implementation_evidence_fingerprint,
            implementation_fragment_count,
            callback_placement_identity_fingerprint,
            compiler_text_validation,
            compiler_function_validation,
            compiler_entry_footprint_binding,
            inventory,
            boundary_placement_binding_fingerprint,
        })
    }

    pub fn validate_identity(&self) -> Result<(), Diagnostic> {
        if self.marker != FINAL_FOOTPRINT_CERTIFICATE_MARKER {
            return Err(Diagnostic::error(
                "unsupported final footprint certificate marker",
            ));
        }
        self.coverage.validate_normalized()?;
        if !self.compiler_text_validation.has_valid_derivation_digest() {
            return Err(Diagnostic::error(
                "compiler text validation evidence has an invalid strong derivation digest",
            ));
        }
        if self
            .compiler_function_validation
            .body_specification_instruction_count
            > 0
            && self.boundary_contract_fingerprint
                != Some(
                    self.compiler_function_validation
                        .body_specification_boundary_contract_fingerprint,
                )
        {
            return Err(Diagnostic::error(
                "final body-specification footprint evidence names a different boundary contract",
            ));
        }
        if self
            .compiler_function_validation
            .fixed_mechanics_instruction_count
            > 0
            && self.boundary_contract_fingerprint
                != Some(
                    self.compiler_function_validation
                        .fixed_mechanics_boundary_contract_fingerprint,
                )
        {
            return Err(Diagnostic::error(
                "final call-return footprint evidence names a different boundary contract",
            ));
        }
        if self.coverage.region_enumeration_complete && !self.inventory.unclassified_gaps.is_empty()
        {
            return Err(Diagnostic::error(
                "region-complete final footprint certificate retains executable gaps",
            ));
        }
        validate_entry_footprint_binding(
            self.boundary_contract_fingerprint,
            self.implementation_evidence_fingerprint,
            self.compiler_function_validation,
            self.compiler_entry_footprint_binding,
            &self.inventory,
        )?;
        let expected_coverage = self.coverage.fingerprint();
        if self.coverage_fingerprint != expected_coverage {
            return Err(Diagnostic::error(
                "final footprint certificate coverage fingerprint mismatch",
            ));
        }
        let expected_binding = placement_binding_fingerprint(
            self.boundary_contract_fingerprint,
            self.implementation_evidence_fingerprint,
            self.callback_placement_identity_fingerprint,
            self.compiler_text_validation.derivation_fingerprint,
            self.compiler_function_validation.evidence_fingerprint(),
            self.compiler_entry_footprint_binding
                .map(|binding| binding.evidence_fingerprint)
                .unwrap_or_default(),
            self.inventory.inventory_fingerprint,
        );
        if self.boundary_placement_binding_fingerprint != expected_binding {
            return Err(Diagnostic::error(
                "final footprint certificate placement binding mismatch",
            ));
        }
        let expected_certificate = certificate_fingerprint(
            expected_coverage,
            expected_binding,
            self.compiler_text_validation.derivation_fingerprint,
            self.compiler_function_validation.evidence_fingerprint(),
            self.compiler_entry_footprint_binding
                .map(|binding| binding.evidence_fingerprint)
                .unwrap_or_default(),
            self.inventory.inventory_fingerprint,
        );
        if self.certificate_fingerprint != expected_certificate {
            return Err(Diagnostic::error(
                "final footprint certificate identity mismatch",
            ));
        }
        Ok(())
    }
}

fn validate_entry_footprint_binding(
    boundary_contract_fingerprint: Option<u64>,
    implementation_evidence_fingerprint: u64,
    compiler_function_validation: CompilerFunctionValidationEvidence,
    binding: Option<CompilerEntryFootprintBindingEvidence>,
    inventory: &PlacedExecutableRegionInventory,
) -> Result<(), Diagnostic> {
    match (boundary_contract_fingerprint, binding) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(Diagnostic::error(
            "final footprint certificate retains entry-footprint custody without a boundary contract",
        )),
        (Some(_), None) => Err(Diagnostic::error(
            "final footprint certificate lacks exact entry-footprint mutation custody",
        )),
        (Some(_), Some(binding)) => {
            if !binding.validate_identity()
                || binding.footprint_fingerprint != implementation_evidence_fingerprint
                || binding.final_region_binding_fingerprint
                    != compiler_function_validation.final_region_binding_fingerprint
                || binding.resulting_inventory_fingerprint != inventory.inventory_fingerprint
                || binding.prior_inventory_fingerprint == binding.resulting_inventory_fingerprint
            {
                return Err(Diagnostic::error(
                    "final footprint certificate entry-footprint mutation custody drifted",
                ));
            }
            Ok(())
        }
    }
}

fn placement_binding_fingerprint(
    boundary_contract_fingerprint: Option<u64>,
    implementation_evidence_fingerprint: u64,
    callback_placement_identity_fingerprint: u64,
    compiler_text_derivation_fingerprint: u64,
    compiler_function_validation_fingerprint: u64,
    compiler_entry_footprint_binding_fingerprint: u64,
    inventory_fingerprint: u64,
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(
        &mut hash,
        &[
            u8::from(boundary_contract_fingerprint.is_some()),
            0x42,
            0x50,
            0x42,
        ],
    );
    fingerprint_bytes(
        &mut hash,
        &boundary_contract_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &implementation_evidence_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &callback_placement_identity_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_text_derivation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_function_validation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_entry_footprint_binding_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(&mut hash, &inventory_fingerprint.to_le_bytes());
    hash
}

fn certificate_fingerprint(
    coverage_fingerprint: u64,
    boundary_placement_binding_fingerprint: u64,
    compiler_text_derivation_fingerprint: u64,
    compiler_function_validation_fingerprint: u64,
    compiler_entry_footprint_binding_fingerprint: u64,
    inventory_fingerprint: u64,
) -> u64 {
    let mut hash = FNV_OFFSET;
    fingerprint_bytes(&mut hash, FINAL_FOOTPRINT_CERTIFICATE_MARKER.as_bytes());
    fingerprint_bytes(&mut hash, &coverage_fingerprint.to_le_bytes());
    fingerprint_bytes(
        &mut hash,
        &boundary_placement_binding_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_text_derivation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_function_validation_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(
        &mut hash,
        &compiler_entry_footprint_binding_fingerprint.to_le_bytes(),
    );
    fingerprint_bytes(&mut hash, &inventory_fingerprint.to_le_bytes());
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
mod tests {
    use super::*;

    fn entry_footprint_binding() -> CompilerEntryFootprintBindingEvidence {
        let mut binding = CompilerEntryFootprintBindingEvidence {
            entry_region_evidence_fingerprint: 20,
            final_region_binding_fingerprint: 19,
            prior_inventory_fingerprint: 12,
            footprint_fingerprint: 2,
            resulting_inventory_fingerprint: 13,
            evidence_fingerprint: 0,
        };
        binding.evidence_fingerprint = binding.recomputed_evidence_fingerprint();
        binding
    }

    fn compiler_text_validation() -> CompilerTextValidationEvidence {
        let mut evidence = CompilerTextValidationEvidence {
            encoded_text_digest: EncodedCompilerTextDigest::from_digest([1; 32]),
            final_compiler_text_digest: FinalCompilerTextDigest::from_digest([2; 32]),
            relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest::from_digest([3; 32]),
            derivation_digest: CompilerTextDerivationDigest::from_digest([0; 32]),
            encoded_text_fingerprint: 4,
            final_compiler_text_fingerprint: 5,
            relocation_envelope_fingerprint: 6,
            checked_instruction_validation_fingerprint: 7,
            checked_instruction_footprint_fingerprint: 18,
            derivation_fingerprint: 8,
            text_relocation_count: 9,
            checked_instruction_validation_count: 10,
        };
        evidence.derivation_digest = evidence.recomputed_derivation_digest();
        evidence
    }

    fn empty_compiler_text_validation() -> CompilerTextValidationEvidence {
        let mut evidence = CompilerTextValidationEvidence {
            encoded_text_digest: EncodedCompilerTextDigest::from_digest([0; 32]),
            final_compiler_text_digest: FinalCompilerTextDigest::from_digest([0; 32]),
            relocation_envelope_digest: CompilerTextRelocationEnvelopeDigest::from_digest([0; 32]),
            derivation_digest: CompilerTextDerivationDigest::from_digest([0; 32]),
            encoded_text_fingerprint: 0,
            final_compiler_text_fingerprint: 0,
            relocation_envelope_fingerprint: 0,
            checked_instruction_validation_fingerprint: 0,
            checked_instruction_footprint_fingerprint: 0,
            derivation_fingerprint: 0,
            text_relocation_count: 0,
            checked_instruction_validation_count: 0,
        };
        evidence.derivation_digest = evidence.recomputed_derivation_digest();
        evidence
    }

    fn certificate() -> FinalFootprintCertificate {
        FinalFootprintCertificate::current(
            Some(1),
            2,
            3,
            21,
            compiler_text_validation(),
            CompilerFunctionValidationEvidence {
                function_count: 1,
                instruction_count: 2,
                zero_width_instruction_count: 0,
                checked_assembly_instruction_count: 0,
                fixed_mechanics_instruction_count: 2,
                fixed_mechanics_validation_fingerprint: 14,
                fixed_mechanics_boundary_contract_fingerprint: 1,
                fixed_mechanics_footprint_fingerprint: 17,
                body_specification_instruction_count: 3,
                body_specification_validation_fingerprint: 15,
                body_specification_boundary_contract_fingerprint: 1,
                body_specification_footprint_fingerprint: 16,
                composed_footprint_fingerprint: 18,
                final_region_binding_fingerprint: 19,
                validation_fingerprint: 11,
            },
            Some(entry_footprint_binding()),
            PlacedExecutableRegionInventory {
                text_address: 0x1000,
                text_byte_count: 4,
                text_fingerprint: 12,
                inventory_fingerprint: 13,
                regions: Vec::new(),
                unclassified_gaps: Vec::new(),
            },
        )
        .expect("complete certificate")
    }

    #[test]
    fn complete_certificate_binds_coverage_placement_text_and_inventory() {
        let certificate = certificate();
        certificate.validate_identity().expect("valid identity");

        for drifted in [
            {
                let mut value = certificate.clone();
                value.marker = "omega.final-footprint-certificate.stale";
                value
            },
            {
                let mut value = certificate.clone();
                value
                    .coverage
                    .missing_classes
                    .push(FinalFootprintClass::CompilerFunctions);
                value
            },
            {
                let mut value = certificate.clone();
                value.boundary_contract_fingerprint = Some(99);
                value
            },
            {
                let mut value = certificate.clone();
                value.callback_placement_identity_fingerprint = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_text_validation.derivation_fingerprint = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_text_validation.encoded_text_digest =
                    EncodedCompilerTextDigest::from_digest([99; 32]);
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_function_validation.validation_fingerprint = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_function_validation.instruction_count = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value
                    .compiler_function_validation
                    .final_region_binding_fingerprint = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value
                    .compiler_entry_footprint_binding
                    .as_mut()
                    .expect("entry binding")
                    .prior_inventory_fingerprint = 99;
                value
            },
            {
                let mut value = certificate.clone();
                value.compiler_entry_footprint_binding = None;
                value
            },
            {
                let mut value = certificate.clone();
                value.inventory.inventory_fingerprint = 99;
                value
            },
        ] {
            assert!(drifted.validate_identity().is_err());
        }
    }

    #[test]
    fn complete_coverage_classifies_admitted_leaves_as_absent() {
        let coverage = FinalFootprintCoverage::current();
        assert!(
            coverage
                .absent_by_construction_classes
                .contains(&FinalFootprintClass::AdmittedLeaves)
        );
        assert!(
            !coverage
                .missing_classes
                .contains(&FinalFootprintClass::AdmittedLeaves)
        );
        coverage.validate_normalized().expect("normalized coverage");
    }

    #[test]
    fn coverage_rejects_conflicting_class_statuses() {
        let mut coverage = FinalFootprintCoverage::current();
        coverage
            .missing_classes
            .push(FinalFootprintClass::AdmittedLeaves);
        assert!(
            coverage
                .validate_normalized()
                .expect_err("one class cannot be both absent and missing")
                .message
                .contains("both absent-by-construction and missing")
        );
    }

    #[test]
    fn region_complete_certificate_rejects_gaps() {
        let mut inventory = certificate().inventory;
        inventory
            .unclassified_gaps
            .push(crate::PlacedExecutableGap {
                section_offset: 0,
                address: 0x1000,
                byte_count: 1,
                byte_fingerprint: 1,
            });
        assert!(
            FinalFootprintCertificate::current(
                None,
                0,
                0,
                0,
                empty_compiler_text_validation(),
                CompilerFunctionValidationEvidence {
                    function_count: 0,
                    instruction_count: 0,
                    zero_width_instruction_count: 0,
                    checked_assembly_instruction_count: 0,
                    fixed_mechanics_instruction_count: 0,
                    fixed_mechanics_validation_fingerprint: 0,
                    fixed_mechanics_boundary_contract_fingerprint: 0,
                    fixed_mechanics_footprint_fingerprint: 0,
                    body_specification_instruction_count: 0,
                    body_specification_validation_fingerprint: 0,
                    body_specification_boundary_contract_fingerprint: 0,
                    body_specification_footprint_fingerprint: 0,
                    composed_footprint_fingerprint: 0,
                    final_region_binding_fingerprint: 0,
                    validation_fingerprint: 0,
                },
                None,
                inventory,
            )
            .is_err()
        );
    }
}
