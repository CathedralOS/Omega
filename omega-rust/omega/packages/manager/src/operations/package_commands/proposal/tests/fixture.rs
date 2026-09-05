use super::super::*;
use package_source::{GitCommitId, GitTreeId, ImmutableSourceResolution};

pub(super) fn pending() -> PendingPackageChange {
    PendingPackageChange {
        kind: PackageCommandKind::Install,
        before_build: [0xab; 32],
        before_lock: None,
        original_content: [0xcd; 32],
        proposed_build: "machine build(builder: &mut Build) {}\n".into(),
        source: source(TargetProfile::CrossPlatformCli),
        targets: vec![TargetProfile::CrossPlatformCli],
    }
}

pub(super) fn source(target: TargetProfile) -> CanonicalSourceClosureSubject {
    // The one-package Git fixture inputs from
    // resolution/graph/subject/model/tests.rs: git_source("codec", "codec", 1)
    // and root_git_selection. Its construction helpers are private to that
    // owner, so recover the corresponding canonical text through the public
    // API. No resolver, filesystem, or compiler is needed for this codec test.
    let commit = "1".repeat(40);
    let tree = "2".repeat(40);
    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&commit).unwrap(),
        GitTreeId::parse_hex(&tree).unwrap(),
    )
    .unwrap();
    let content = resolution.content().to_hex();
    let selected = format!(
        "source\nname \"codec\"\nlineage github \"cathedralos\" \"codec\"\nresolution git \"{commit}\" \"{tree}\" \"{content}\"\n"
    );
    let identity = target.identity().as_str();
    let text = format!(
        "omega-source-closure 1\ntarget \"{identity}\"\nroot\nrole package\nrequest git \"https://github.com/CathedralOS/codec.git\" \"main\"\nselection root\nselected\n{selected}packages 1\npackage\n{selected}navigation root\nauthored 0\nedges 0\nend\n"
    );
    let subject = CanonicalSourceClosureSubject::recover_text(&text, Default::default()).unwrap();
    assert_eq!(subject.canonical_text(Default::default()).unwrap(), text);
    subject
}
