//! Production readers of the numbered native pipeline. Each exception is one
//! Cargo package consuming one named API. Nested packages have their own identity.
use super::linear_pipeline::NATIVE_STAGES;
use super::{Allowance, Rule, under};

macro_rules! native {
    ($path:literal) => {
        concat!("source-rust/subsystems/compiler/psi-to-native", $path)
    };
}

pub(super) const DRIVER: &str = native!("");
const HOST_PROVIDERS: &str = native!("/pipeline/00_psi-to-admitted-native-inputs/host-providers");
const ENTRY: &str =
    native!("/pipeline/01_admitted-native-inputs-to-target-bound-program/program-entry-contracts");
const ENTRY_PLANNING: &str =
    native!("/pipeline/09_post-allocation-machine-to-framed-machine/entry-planning");
const VERIFIED_INPUT: &str =
    native!("/pipeline/representations/abstract-operations/verified-optimization-input");
const ANALYSES: &str =
    native!("/pipeline/06_selected-instructions-to-selected-instructions/analyses");
const ENCODING: &str = native!("/pipeline/representations/machine-code/encoding-admission");
const AARCH64: &str =
    native!("/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-aarch64");
const X86: &str = native!("/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64");
pub(super) const IMAGE_EMISSION: &str = native!("/pipeline/16_objects-to-image/image-emission");
pub(super) const NATIVE_ARTIFACT: &str = native!("/pipeline/16_objects-to-image/native-artifact");
pub(super) const OBJECT_REPLAY: &str = native!("/replay/published-objects");
pub(super) const PRODUCT_PUBLICATION: &str =
    "source-rust/subsystems/compiler/build/build-output/product-publication";

pub(super) fn scoped_readers() -> Vec<Allowance> {
    let stages = &NATIVE_STAGES;
    let mut readers = vec![
        ("source-rust/subsystems/package-manager", HOST_PROVIDERS),
        (ENTRY_PLANNING, ENTRY),
        (VERIFIED_INPUT, ENTRY),
        (VERIFIED_INPUT, native!("/pipeline/01_admitted-native-inputs-to-target-bound-program/execution-scope")),
        (IMAGE_EMISSION, ENTRY),
        (native!("/saved-inputs"), ENTRY),
        (ENCODING, AARCH64),
        (ENCODING, X86),
        (
            "source-rust/subsystems/interpreter/host/native-sponsor",
            IMAGE_EMISSION,
        ),
        (PRODUCT_PUBLICATION, NATIVE_ARTIFACT),
    ];
    // Published-object replay reconstructs precisely these witnesses.
    for dependency in [
        stages[4],
        stages[6],
        ANALYSES,
        stages[7],
        stages[8],
        stages[9],
        stages[13],
        IMAGE_EMISSION,
    ] {
        readers.push((OBJECT_REPLAY, dependency));
    }
    readers
        .into_iter()
        .map(|(owner, dependency)| Allowance { owner, dependency })
        .collect()
}

pub(super) fn rules() -> Vec<Rule> {
    let stages = &NATIVE_STAGES;
    let readers = scoped_readers();
    let mut rules = Vec::new();
    for (index, &stage) in stages.iter().enumerate() {
        let allowed = readers
            .iter()
            .copied()
            .filter(|edge| under(edge.dependency, &[stage]))
            .collect();
        rules.push(Rule {
            owners: vec![""],
            forbidden: vec![stage],
            forbidden_packages: vec![],
            reason: "native stages are consumed by the native driver, their successor, or an explicitly scoped reader",
            except_package: Some(DRIVER),
            except_owners: stages[index..(index + 2).min(stages.len())].to_vec(),
            allowed,
        });
    }
    let earlier_forms = vec![
        native!("/pipeline/representations/abstract-operations"),
        native!("/pipeline/representations/target-operations"),
        native!("/pipeline/representations/target-operations/legalized-operations"),
    ];
    // The image package still contains the requested object correspondence audit.
    // Its accessor and validator call sites are also restricted by boundary.rs.
    let retained_image_readers = earlier_forms
        .iter()
        .map(|&dependency| Allowance {
            owner: IMAGE_EMISSION,
            dependency,
        })
        .collect();
    rules.push(Rule {
        owners: stages[6..].to_vec(),
        forbidden: vec![],
        forbidden_packages: earlier_forms,
        reason: "selected and later stages must read operands from the owning vocabulary",
        except_package: None,
        except_owners: vec![],
        allowed: retained_image_readers,
    });
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::tests::pkg;
    use crate::boundary::{Metadata, forbidden_dependency_violations};

    #[test]
    fn physical_stages_can_read_shared_operands_without_importing_the_earlier_program() {
        let target = native!("/pipeline/representations/target-operations");
        let vocabulary = native!("/pipeline/representations/target-operations/target-vocabulary");
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg(
                    "frame",
                    stages()[9],
                    &[("record", vocabulary, None), ("earlier", target, None)],
                ),
                pkg("audit", IMAGE_EMISSION, &[("earlier", target, None)]),
                pkg(
                    "artifact",
                    NATIVE_ARTIFACT,
                    &[("earlier", target, Some("build"))],
                ),
            ],
        };
        let violations = forbidden_dependency_violations(&metadata, &rules()).unwrap();
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(
            violations
                .iter()
                .any(|message| message.starts_with("frame -> target-operations:"))
        );
        assert!(
            violations
                .iter()
                .any(|message| message.starts_with("artifact -> target-operations:"))
        );
    }
}
