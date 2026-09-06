//! Selected analysis data and canonical identities do not carry validation authority.

use super::{repository, rust_source};

const LIVENESS_TYPES: &[&str] = &[
    "LivenessPosition",
    "LivenessIdentity",
    "LivenessPlan",
    "FunctionLiveness",
    "EntryDefinition",
    "OperandPosition",
    "BlockLiveness",
    "InstructionLiveness",
    "SuccessorLiveness",
];
const LIVE_RANGE_TYPES: &[&str] = &[
    "LiveRangePoint",
    "LiveRangeIdentity",
    "LiveRangePlan",
    "FunctionLiveRanges",
    "EdgeRegisterTransfer",
    "EarlyClobberConstraint",
    "EarlyClobberUse",
    "DistinctUseDefTie",
    "BlockPointDomain",
    "VirtualLiveRange",
    "VirtualOccurrence",
    "VirtualFixedConstraintSite",
    "VirtualFixedConstraint",
    "LiveRangeFragment",
    "LiveRangeEdgeConnector",
    "ArchitecturalUnitLiveRange",
    "ArchitecturalUnitAction",
    "ArchitecturalUnitActionKind",
    "VirtualInterference",
];

#[test]
fn selected_analysis_schemas_and_identities_have_one_representation_owner() {
    let root = repository();
    let owner = root.join("omega-rust/omega/representations/selected-instructions");
    let transform =
        root.join("omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src");
    let stage = rust_source(&transform);
    let homes = rust_source(&root.join("omega-rust/omega/representations/register-homes/src"));
    let representation = rust_source(&owner.join("src"));
    let program_root = std::fs::read_to_string(owner.join("src/selected_instructions.rs")).unwrap();
    for (module, types, function, domain) in [
        (
            "liveness",
            LIVENESS_TYPES,
            "liveness_identity",
            "omega.terminal-register-liveness.v8",
        ),
        (
            "live_ranges",
            LIVE_RANGE_TYPES,
            "live_range_identity",
            "omega.terminal-live-range-fragments.v9",
        ),
    ] {
        assert!(program_root.contains(&format!("mod {module};")));
        assert!(program_root.contains(&format!("pub use {module}::")));
        let schema =
            std::fs::read_to_string(owner.join(format!("src/selected_instructions/{module}.rs")))
                .unwrap();
        for name in types {
            let declarations = [
                format!("pub struct {name} {{"),
                format!("pub struct {name}("),
                format!("pub enum {name} {{"),
            ];
            assert_eq!(
                declarations
                    .iter()
                    .map(|declaration| schema.matches(declaration).count())
                    .sum::<usize>(),
                1,
                "missing or duplicate {name}"
            );
            for other in [&stage, &homes] {
                assert!(
                    declarations
                        .iter()
                        .all(|declaration| !other.contains(declaration)),
                    "wrong owner of {name}"
                );
                assert!(!other.contains(&format!("pub type {name} ")));
            }
        }
        let identity = std::fs::read_to_string(
            owner.join(format!("src/selected_instructions/{module}/identity.rs")),
        )
        .unwrap();
        assert!(identity.contains(&format!("pub fn {function}(")));
        assert!(identity.contains(domain));
        assert_eq!(representation.matches(domain).count(), 1);
        assert!(
            !stage.contains(domain),
            "transform duplicates canonical {module} encoding"
        );
        assert!(!stage.contains(&format!("pub fn {function}(")));
        assert!(
            !transform
                .join(format!("analyses/{module}/identity.rs"))
                .exists()
        );
        assert!(!schema.contains("ValidationReceipt"));
        assert!(!schema.contains("Validated"));
    }
    let manifest = std::fs::read_to_string(owner.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("/pipeline/"));
    assert!(!manifest.contains("register-homes"));
    assert!(!homes.contains("pub use selected_instructions::LiveRangeIdentity"));
}

#[test]
fn selected_analysis_validation_seals_stay_in_the_transform() {
    let root = repository();
    let owner =
        root.join("omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src");
    let representation =
        rust_source(&root.join("omega-rust/omega/representations/selected-instructions/src"));
    for (module, plan, validated, receipt, error) in [
        (
            "liveness",
            "LivenessPlan",
            "ValidatedLiveness",
            "LivenessValidationReceipt",
            "LivenessError",
        ),
        (
            "live_ranges",
            "LiveRangePlan",
            "ValidatedLiveRanges",
            "LiveRangeValidationReceipt",
            "LiveRangeError",
        ),
    ] {
        let model =
            std::fs::read_to_string(owner.join(format!("analyses/{module}/model.rs"))).unwrap();
        for declaration in [
            format!("pub struct {validated} {{"),
            format!("pub struct {receipt} {{"),
            format!("pub enum {error} {{"),
        ] {
            assert!(model.contains(&declaration));
            assert!(!representation.contains(&declaration));
        }
        assert!(model.contains(&format!("pub(crate) plan: std::sync::Arc<{plan}>")));
        assert!(model.contains(&format!("pub(crate) receipt: {receipt}")));
        assert!(!model.contains("pub plan:"));
        assert!(!model.contains("pub receipt:"));
        assert!(!model.contains("pub fn new("));
        assert!(!model.contains("pub const fn new("));
        assert!(!model.contains("pub fn from_plan("));
        let receipt_fields = model
            .split(&format!("pub struct {receipt} {{"))
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        assert!(
            receipt_fields
                .lines()
                .all(|line| !line.trim().starts_with("pub "))
        );
    }
    let stage = rust_source(&owner);
    for (start, _) in stage.match_indices("pub use ") {
        let statement = stage[start..].split(';').next().unwrap();
        let words = statement
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .collect::<Vec<_>>();
        for name in LIVENESS_TYPES
            .iter()
            .chain(LIVE_RANGE_TYPES)
            .copied()
            .chain(["liveness_identity", "live_range_identity"])
        {
            assert!(
                !words.contains(&name),
                "transform publicly reexports raw selected analysis: {statement}"
            );
        }
        assert!(!statement.contains("selected_instructions::*"));
        assert!(!statement.contains("selected_instructions::liveness"));
        assert!(!statement.contains("selected_instructions::live_ranges"));
    }
}
