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
const ANALYSES: &str =
    native!("/pipeline/06_selected-instructions-to-selected-instructions/analyses");
pub(super) const IMAGE_EMISSION: &str = native!("/pipeline/15_objects-to-image/image-emission");
pub(super) const NATIVE_ARTIFACT: &str = native!("/pipeline/15_objects-to-image/native-artifact");
pub(super) const OBJECT_REPLAY: &str = native!("/verification/published-objects");
pub(super) const PRODUCT_PUBLICATION: &str =
    "source-rust/subsystems/compiler/build/publication";
/// Program forms instruction selection consumes. Later stages read the facts
/// they need from the target vocabulary and the physical program instead.
const EARLIER_FORMS: [&str; 4] = [
    native!("/representations/abstract-operations"),
    native!("/representations/abstract-operations/optimization-unit-semantics"),
    native!("/representations/target-operations"),
    native!("/representations/target-operations/legalized-operations"),
];

pub(super) fn scoped_readers() -> Vec<Allowance> {
    let stages = &NATIVE_STAGES;
    let mut readers = vec![
        ("source-rust/subsystems/package-manager", HOST_PROVIDERS),
        // Fixed-width entry layouts already contain the final call and relocation fields.
        (stages[12], stages[10]),
        (
            "source-rust/subsystems/interpreter/host/native-sponsor",
            IMAGE_EMISSION,
        ),
        (PRODUCT_PUBLICATION, NATIVE_ARTIFACT),
        (super::RETAINED_IMAGE_RUNTIME, NATIVE_ARTIFACT),
    ];
    // Published-object replay reconstructs precisely these witnesses.
    for dependency in [
        stages[3],
        stages[5],
        ANALYSES,
        stages[6],
        stages[7],
        stages[8],
        IMAGE_EMISSION,
    ] {
        readers.push((OBJECT_REPLAY, dependency));
    }
    readers.extend(super::linear_pipeline::legacy_representation_links_into(
        stages,
    ));
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
    // Stage 02's abstract program and stage 04's target operations stop at
    // instruction selection. The requested object audit that rejoins them to an
    // emitted object lives in the published-object verifier, outside the pipeline,
    // so no later stage package, image emission included, may depend on them.
    rules.push(Rule {
        owners: stages[5..].to_vec(),
        forbidden: vec![],
        forbidden_packages: EARLIER_FORMS.to_vec(),
        reason: "selected and later stages must read operands from the owning vocabulary",
        except_package: None,
        except_owners: vec![],
        allowed: vec![],
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
        let target = native!("/representations/target-operations");
        let vocabulary = native!("/representations/target-operations/target-vocabulary");
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg(
                    "frame",
                    NATIVE_STAGES[8],
                    &[("record", vocabulary, None), ("earlier", target, None)],
                ),
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

    /// Image emission reads the object stage's handoff and the target vocabulary.
    /// Its unit tests may still build abstract fixtures, so only dev edges pass.
    #[test]
    fn image_emission_cannot_import_any_earlier_program_form() {
        let vocabulary = native!("/representations/target-operations/target-vocabulary");
        let mut production = vec![("record", vocabulary, None)];
        production.extend(EARLIER_FORMS.iter().map(|&form| ("earlier", form, None)));
        let fixtures: Vec<_> = EARLIER_FORMS
            .iter()
            .map(|&form| ("fixture", form, Some("dev")))
            .collect();
        let metadata = Metadata {
            workspace_root: "/repo".into(),
            packages: vec![
                pkg("image-emission", IMAGE_EMISSION, &production),
                pkg("image-tests", IMAGE_EMISSION, &fixtures),
                pkg(
                    "audit",
                    OBJECT_REPLAY,
                    &[
                        ("earlier", EARLIER_FORMS[0], None),
                        ("target", EARLIER_FORMS[2], None),
                    ],
                ),
            ],
        };
        let violations = forbidden_dependency_violations(&metadata, &rules()).unwrap();
        assert_eq!(violations.len(), EARLIER_FORMS.len(), "{violations:?}");
        assert!(
            violations
                .iter()
                .all(|message| message.starts_with("image-emission -> "))
        );
    }
}
