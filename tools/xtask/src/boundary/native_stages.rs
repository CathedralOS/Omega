//! Production readers of the numbered native pipeline. Each exception is one
//! Cargo package consuming one named API. Nested packages have their own identity.
use super::linear_pipeline::STAGES;
use super::{Allowance, Rule, under};

macro_rules! native {
    ($path:literal) => {
        concat!("source-rust/subsystems/compiler/psi-to-native", $path)
    };
}

pub(super) const DRIVER: &str = native!("");
const HOST_PROVIDERS: &str = native!("/pipeline/00_psi-to-admitted-native-inputs/host-providers");
const DEMAND: &str = native!("/pipeline/00_psi-to-admitted-native-inputs/provider-demand");
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
const OBJECT_REPLAY: &str = native!("/replay/published-objects");
const PSI_PUBLICATION: &str =
    "source-rust/subsystems/compiler/build/package-compilation/psi-publication";
pub(super) const PRODUCT_PUBLICATION: &str =
    "source-rust/subsystems/compiler/build/build-output/product-publication";

pub(super) fn rules() -> Vec<Rule> {
    // The shared ordering also contains the source stages and file publication.
    let stages: Vec<_> = std::iter::once(STAGES[0])
        .chain(STAGES[3..19].iter().copied())
        .collect();
    let mut readers = vec![
        ("source-rust/subsystems/package-manager", HOST_PROVIDERS),
        (PSI_PUBLICATION, HOST_PROVIDERS),
        (PSI_PUBLICATION, DEMAND),
        (ENTRY_PLANNING, ENTRY),
        (VERIFIED_INPUT, ENTRY),
        (IMAGE_EMISSION, ENTRY),
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
    stages.iter().enumerate().map(|(index, &stage)| Rule {
        owners: vec![""],
        forbidden: vec![stage],
        reason: "native stages are consumed by the native driver, their successor, or an explicitly scoped reader",
        except_package: Some(DRIVER),
        except_owners: stages[index..(index + 2).min(stages.len())].to_vec(),
        allowed: readers.iter().filter(|(_, dependency)| under(dependency, &[stage]))
            .map(|&(owner, dependency)| Allowance { owner, dependency }).collect(),
    }).collect()
}
