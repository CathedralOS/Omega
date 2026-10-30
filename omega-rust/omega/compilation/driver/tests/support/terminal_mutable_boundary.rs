//! Structural paths a mutable-boundary provision reaches into: the record
//! fields a source-produced Build call retains across host effects and fuel
//! pauses.

use semantic_vocabulary::CanonicalStructuralPathSegment as Path;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_interpreter::TerminalProgram;

pub(super) fn field(artifact: &CanonicalTerminalArtifact, name: &str) -> Path {
    let program = TerminalProgram::borrow(artifact.view());
    let entry = program.entry().unwrap();
    let root = entry.structural_parameters[0].structural_type;
    let declaration = artifact
        .semantic_module()
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        panic!("record entry");
    };
    let identities = &artifact.semantic_module().identities;
    Path::Field(
        fields
            .iter()
            .find(|field| identities.text(field.identity) == Some(name))
            .unwrap()
            .id,
    )
}
