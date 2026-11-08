//! Encoding vocabulary shared by both halves of the compiled-review cache:
//! the input digests and build-record codec in this crate, and the package
//! manager's record key and record codec.
//!
//! Each tag table exists once, so a digest and a record can never disagree
//! about the byte a value takes. The tags are persisted wire format: an
//! existing tag never changes meaning, and a new variant takes a new tag.
//! Length framing itself (`push_framed`, `ObservationCursor`) is the checked
//! interpreter's observation codec.

use crate::{AcceptedSemanticBindingRole, BuildDeclarationKind};
use build_declarations::DependencyPurpose;
use effects::TerminalAuthorityClass;
use filesystem_authority::observation_codec::ObservationCursor;

/// Write `0` for an absent value, or `1` followed by the encoded value.
pub fn encode_optional<T>(value: Option<&T>, out: &mut Vec<u8>, encode: impl Fn(&T, &mut Vec<u8>)) {
    match value {
        Some(value) => {
            out.push(1);
            encode(value, out);
        }
        None => out.push(0),
    }
}

/// Read what `encode_optional` wrote. Any presence byte other than `0` or
/// `1`, or a present value that fails to decode, is a decode failure.
pub fn recover_optional<'a, T>(
    cursor: &mut ObservationCursor<'a>,
    decode: impl Fn(&mut ObservationCursor<'a>) -> Option<T>,
) -> Option<Option<T>> {
    match cursor.u8()? {
        0 => Some(None),
        1 => decode(cursor).map(Some),
        _ => None,
    }
}

pub fn purpose_tag(purpose: DependencyPurpose) -> u8 {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}

pub fn recover_purpose(cursor: &mut ObservationCursor<'_>) -> Option<DependencyPurpose> {
    match cursor.u8()? {
        0 => Some(DependencyPurpose::Product),
        1 => Some(DependencyPurpose::Build),
        _ => None,
    }
}

pub fn declaration_kind_tag(kind: BuildDeclarationKind) -> u8 {
    match kind {
        BuildDeclarationKind::Package => 0,
        BuildDeclarationKind::Application => 1,
        BuildDeclarationKind::Workspace => 2,
    }
}

pub fn recover_declaration_kind(
    cursor: &mut ObservationCursor<'_>,
) -> Option<BuildDeclarationKind> {
    match cursor.u8()? {
        0 => Some(BuildDeclarationKind::Package),
        1 => Some(BuildDeclarationKind::Application),
        2 => Some(BuildDeclarationKind::Workspace),
        _ => None,
    }
}

pub fn semantic_binding_role_tag(role: AcceptedSemanticBindingRole) -> u8 {
    match role {
        AcceptedSemanticBindingRole::ConsoleExitProcessI32 => 0,
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32 => 1,
        AcceptedSemanticBindingRole::FilesystemHostService => 2,
        AcceptedSemanticBindingRole::TimeHostService => 3,
        AcceptedSemanticBindingRole::UefiX64ProgramEntry => 4,
        AcceptedSemanticBindingRole::MacosArm64ProgramEntry => 5,
        AcceptedSemanticBindingRole::MacosX64ProgramEntry => 6,
        AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry => 7,
        AcceptedSemanticBindingRole::LinuxArm64ProgramEntry => 8,
        AcceptedSemanticBindingRole::WindowsX64ProgramEntry => 9,
    }
}

pub fn recover_semantic_binding_role(
    cursor: &mut ObservationCursor<'_>,
) -> Option<AcceptedSemanticBindingRole> {
    Some(match cursor.u8()? {
        0 => AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        1 => AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
        2 => AcceptedSemanticBindingRole::FilesystemHostService,
        3 => AcceptedSemanticBindingRole::TimeHostService,
        4 => AcceptedSemanticBindingRole::UefiX64ProgramEntry,
        5 => AcceptedSemanticBindingRole::MacosArm64ProgramEntry,
        6 => AcceptedSemanticBindingRole::MacosX64ProgramEntry,
        7 => AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry,
        8 => AcceptedSemanticBindingRole::LinuxArm64ProgramEntry,
        9 => AcceptedSemanticBindingRole::WindowsX64ProgramEntry,
        _ => return None,
    })
}

pub fn authority_class_tag(class: TerminalAuthorityClass) -> u8 {
    class.canonical_tag()
}

pub fn recover_authority_class(
    cursor: &mut ObservationCursor<'_>,
) -> Option<TerminalAuthorityClass> {
    let tag = cursor.u8()?;
    TerminalAuthorityClass::ALL
        .into_iter()
        .find(|class| class.canonical_tag() == tag)
}
