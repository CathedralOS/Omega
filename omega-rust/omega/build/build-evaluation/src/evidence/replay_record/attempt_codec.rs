//! The wire encoding of one replay attempt: the shape structs, the byte
//! encoder and decoder, and the tag values every lane carries.
//!
//! This file encodes and decodes replay activations. `attempt_encoding.rs`
//! encodes one attempt, `attempt_shapes.rs` carries the decoded attempt
//! shapes, `attempt_decoding.rs` decodes one attempt, `wire.rs` the byte
//! encoder and decoder and `first_rung_validation_tests.rs` the codec
//! tests.

mod attempt_decoding;
mod attempt_encoding;
mod attempt_shapes;
#[cfg(test)]
mod first_rung_validation_tests;
mod wire;

pub(crate) use attempt_decoding::decode_attempt;
pub(crate) use attempt_encoding::encode_attempt;
pub(crate) use attempt_shapes::{
    AttemptShape, ShapeAuthorizedPath, ShapeLogicalInput, ShapeLogicalInputResolution,
    ShapeMutableI64, ShapeRefusal, ShapeResult, ShapeReturnedPath, ShapeRootedPath, ShapeScalar,
};
#[cfg(test)]
pub(crate) use attempt_shapes::{
    ShapeLogicalOutput, ShapeMetadata, ShapeMutableBytes, ShapeObservedRegion,
};
pub(crate) use wire::{Decoder, Encoder};

use crate::BuildReplayActivation;
use crate::evidence::replay_record::BuildFilesystemReplayRecordError;

pub(crate) fn encode_replay_activation(
    encoder: &mut Encoder,
    activation: BuildReplayActivation,
) -> Result<(), BuildFilesystemReplayRecordError> {
    match activation.root_package_identity() {
        None => encoder.byte(0),
        Some(identity) => {
            encoder.byte(1);
            encoder.fixed(&identity.digest());
        }
    }
    encoder.byte(match activation.root_role() {
        None => 0,
        Some(package_compilation::BuildDeclarationKind::Package) => 1,
        Some(package_compilation::BuildDeclarationKind::Application) => 2,
        Some(package_compilation::BuildDeclarationKind::Workspace) => 3,
    });
    encode_optional_target_profile(encoder, activation.selected_target_profile())?;
    encode_optional_target_profile(encoder, activation.build_execution_profile())
}

fn encode_optional_target_profile(
    encoder: &mut Encoder,
    profile: Option<target::TargetProfile>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    match profile {
        None => encoder.byte(0),
        Some(profile) => {
            encoder.byte(1);
            encoder.bytes(profile.target_name().as_bytes())?;
        }
    }
    Ok(())
}

pub(crate) fn decode_replay_activation(
    decoder: &mut Decoder<'_>,
) -> Result<BuildReplayActivation, BuildFilesystemReplayRecordError> {
    let root_package_identity = match decoder.byte()? {
        0 => None,
        1 => Some(
            semantic_vocabulary::PackageKeyIdentity::from_digest(decoder.array_32()?).ok_or_else(
                || {
                    BuildFilesystemReplayRecordError::new(
                        "invalid replay activation root package identity",
                    )
                },
            )?,
        ),
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "invalid replay activation root package tag",
            ));
        }
    };
    let root_role = match decoder.byte()? {
        0 => None,
        1 => Some(package_compilation::BuildDeclarationKind::Package),
        2 => Some(package_compilation::BuildDeclarationKind::Application),
        3 => Some(package_compilation::BuildDeclarationKind::Workspace),
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "invalid replay activation root role tag",
            ));
        }
    };
    let selected_target_profile =
        decode_optional_target_profile(decoder, &SELECTED_TARGET_PROFILE_REJECTIONS)?;
    let build_execution_profile =
        decode_optional_target_profile(decoder, &BUILD_EXECUTION_PROFILE_REJECTIONS)?;
    Ok(BuildReplayActivation {
        root_package_identity,
        root_role,
        selected_target_profile,
        build_execution_profile,
    })
}

/// The rejections one optional target-profile activation member decodes
/// with, so a corrupt record names the member it failed on.
struct ProfileMemberRejections {
    encoding: &'static str,
    profile: &'static str,
    tag: &'static str,
}

const SELECTED_TARGET_PROFILE_REJECTIONS: ProfileMemberRejections = ProfileMemberRejections {
    encoding: "invalid replay activation target profile encoding",
    profile: "invalid replay activation target profile",
    tag: "invalid replay activation target profile tag",
};

const BUILD_EXECUTION_PROFILE_REJECTIONS: ProfileMemberRejections = ProfileMemberRejections {
    encoding: "invalid replay activation build execution profile encoding",
    profile: "invalid replay activation build execution profile",
    tag: "invalid replay activation build execution profile tag",
};

/// Decode one optional canonical target name.
fn decode_optional_target_profile(
    decoder: &mut Decoder<'_>,
    rejections: &ProfileMemberRejections,
) -> Result<Option<target::TargetProfile>, BuildFilesystemReplayRecordError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => {
            let name = std::str::from_utf8(decoder.bytes()?)
                .map_err(|_| BuildFilesystemReplayRecordError::new(rejections.encoding))?;
            target::TargetProfile::from_canonical_target_name(name)
                .map(Some)
                .map_err(|_| BuildFilesystemReplayRecordError::new(rejections.profile))
        }
        _ => Err(BuildFilesystemReplayRecordError::new(rejections.tag)),
    }
}
