//! Allocation facts and codecs are data; only the transform seals admission.

use super::{repository, rust_source};

struct Family {
    source: &'static str,
    destination: &'static str,
    plan: &'static str,
    identity: &'static str,
    domain: &'static str,
    validated: &'static str,
    receipt: &'static str,
}

const FAMILIES: &[Family] = &[
    Family {
        source: "allocator_availability",
        destination: "constraints/allocator_availability",
        plan: "AllocatorAvailabilityPlan",
        identity: "allocator_availability_identity",
        domain: "omega.terminal-allocator-availability.v1",
        validated: "ValidatedAllocatorAvailability",
        receipt: "AllocatorAvailabilityValidationReceipt",
    },
    Family {
        source: "allocation_legality",
        destination: "constraints/allocation_legality",
        plan: "AllocationLegalityPlan",
        identity: "allocation_legality_identity",
        domain: "omega.terminal-allocation-legality.v5",
        validated: "ValidatedAllocationLegality",
        receipt: "AllocationLegalityValidationReceipt",
    },
    Family {
        source: "fixed_precolored_intervals",
        destination: "constraints/fixed_precolored_intervals",
        plan: "FixedPrecoloredIntervalPlan",
        identity: "fixed_precolored_interval_plan_identity",
        domain: "omega.fixed-precolored-point-intervals.v1",
        validated: "ValidatedFixedPrecoloredIntervals",
        receipt: "FixedPrecoloredIntervalValidationReceipt",
    },
    Family {
        source: "fixed_precolored_split_requirements",
        destination: "constraints/fixed_precolored_split_requirements",
        plan: "FixedPrecoloredSplitRequirementPlan",
        identity: "fixed_precolored_split_requirement_plan_identity",
        domain: "omega.fixed-precolored-split-requirements.v1",
        validated: "ValidatedFixedPrecoloredSplitRequirements",
        receipt: "FixedPrecoloredSplitRequirementValidationReceipt",
    },
    Family {
        source: "fixed_precolored_segment_homes",
        destination: "storage/fixed_precolored_segment_homes",
        plan: "FixedPrecoloredSegmentHomePlan",
        identity: "fixed_precolored_segment_home_plan_identity",
        domain: "omega.fixed-precolored-segment-homes.v1",
        validated: "ValidatedFixedPrecoloredSegmentHomes",
        receipt: "FixedPrecoloredSegmentHomeValidationReceipt",
    },
    Family {
        source: "spill_choice",
        destination: "recovery/spill_choice",
        plan: "SpillChoicePlan",
        identity: "spill_choice_identity",
        domain: "omega.terminal-spill-choices.v2",
        validated: "ValidatedSpillChoices",
        receipt: "SpillChoiceValidationReceipt",
    },
    Family {
        source: "recovery_classification",
        destination: "recovery/classification",
        plan: "RecoveryClassificationPlan",
        identity: "recovery_classification_identity",
        domain: "omega.terminal-recovery-classification.v3",
        validated: "ValidatedRecoveryClassifications",
        receipt: "RecoveryClassificationValidationReceipt",
    },
];

#[test]
fn allocation_analysis_data_and_canonical_encoders_have_one_owner() {
    let root = repository();
    let owner = root.join("omega-rust/omega/representations/register-homes/src/register_homes");
    let stage =
        root.join("omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src");
    let stage_text = rust_source(&stage);
    let allocator = rust_source(
        &root.join("omega-rust/omega/pipeline/selected-instructions-to-register-homes/src"),
    );
    let representation = rust_source(&owner);
    assert!(!owner.join("evidence.rs").exists());
    let mut raw_names = Vec::new();
    for family in FAMILIES {
        let schema =
            std::fs::read_to_string(owner.join(format!("{}.rs", family.destination))).unwrap();
        assert!(schema.contains(&format!("pub struct {} {{", family.plan)));
        // Inspect declarations rather than maintaining a second list of every row.
        for declaration in schema
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub struct ") || line.starts_with("pub enum "))
        {
            let name = declaration
                .split_whitespace()
                .nth(2)
                .unwrap()
                .split(['(', '{', '<'])
                .next()
                .unwrap();
            let prefix = if declaration.starts_with("pub struct ") {
                "pub struct "
            } else {
                "pub enum "
            };
            let declaration = format!("{prefix}{name}");
            for consumer in [&stage_text, &allocator] {
                assert!(
                    !consumer.contains(&format!("{declaration} {{")),
                    "duplicate {name}"
                );
                assert!(
                    !consumer.contains(&format!("{declaration}(")),
                    "duplicate {name}"
                );
                assert!(
                    !consumer.contains(&format!("pub type {name} ")),
                    "alias {name}"
                );
            }
            raw_names.push(name.to_owned());
        }
        assert!(!schema.contains("pub struct Validated"));
        assert!(!schema.contains("ValidationReceipt"));
        let identity =
            std::fs::read_to_string(owner.join(format!("{}/identity.rs", family.destination)))
                .unwrap();
        assert!(identity.contains(&format!("pub fn {}(", family.identity)));
        assert!(identity.contains(family.domain));
        assert_eq!(representation.matches(family.domain).count(), 1);
        assert!(!stage_text.contains(family.domain));
        assert!(
            !stage
                .join(format!("analyses/{}/identity.rs", family.source))
                .exists()
        );
        raw_names.push(family.identity.to_owned());
    }
    for consumer in [&stage_text, &allocator] {
        for (start, _) in consumer.match_indices("pub use ") {
            let statement = consumer[start..].split(';').next().unwrap();
            let words = statement
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .collect::<Vec<_>>();
            assert!(
                raw_names.iter().all(|name| !words.contains(&name.as_str())),
                "raw allocation data reexported by transform: {statement}"
            );
            assert!(!statement.contains("register_homes::*"));
            for module in [
                "constraints",
                "recovery",
                "storage::fixed_precolored_segment_homes",
            ] {
                assert!(!statement.contains(&format!("register_homes::{module}")));
            }
        }
    }
}

#[test]
fn allocation_analysis_admission_stays_sealed_in_the_transform() {
    let root = repository();
    let representation =
        rust_source(&root.join("omega-rust/omega/representations/register-homes/src"));
    let stage = root.join(
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses",
    );
    for family in FAMILIES {
        let model =
            std::fs::read_to_string(stage.join(format!("{}/model.rs", family.source))).unwrap();
        for name in [family.validated, family.receipt] {
            let declaration = format!("pub struct {name} {{");
            assert!(model.contains(&declaration));
            assert!(!representation.contains(&declaration));
            let fields = model
                .split(&declaration)
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            assert!(
                fields.lines().all(|line| !line.trim().starts_with("pub ")),
                "unsealed {name}"
            );
        }
        assert!(model.contains("pub(crate) plan:"));
        assert!(model.contains(&format!("pub(crate) receipt: {}", family.receipt)));
        for constructor in ["pub fn new(", "pub const fn new(", "pub fn from_plan("] {
            assert!(
                !model.contains(constructor),
                "unsealed constructor in {}",
                family.source
            );
        }
    }
}

#[test]
fn allocation_recovery_codecs_decode_only_raw_plans() {
    let root = repository();
    let owner =
        root.join("omega-rust/omega/representations/register-homes/src/register_homes/recovery");
    let stage = root.join(
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses",
    );
    for (module, source, plan, error) in [
        (
            "spill_choice",
            "spill_choice",
            "SpillChoicePlan",
            "SpillChoiceDecodeError",
        ),
        (
            "classification",
            "recovery_classification",
            "RecoveryClassificationPlan",
            "RecoveryClassificationDecodeError",
        ),
    ] {
        let mut data = std::fs::read_to_string(owner.join(format!("{module}.rs"))).unwrap();
        data.push_str(&rust_source(&owner.join(module)));
        assert!(data.contains(&format!("impl {plan}")));
        assert!(data.contains("pub fn encode(&self) -> Vec<u8>"));
        assert!(data.contains(&format!(
            "pub fn decode(encoded: &[u8]) -> Result<Self, {error}>"
        )));
        assert!(data.contains(&format!("pub enum {error} {{")));
        assert!(!data.contains("Validated"));
        assert!(!data.contains("ValidationReceipt"));
        let old = rust_source(&stage.join(source));
        assert!(!old.contains(&format!("impl {plan}")));
        assert!(!old.contains(&format!("pub enum {error} {{")));
        assert!(!stage.join(format!("{source}/persistence.rs")).exists());
    }
}
