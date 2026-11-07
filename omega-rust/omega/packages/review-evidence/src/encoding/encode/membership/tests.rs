// Unit tests for membership descent over rendered identity text, limit exhaustion, const operands, and malformed-identity excerpts.

use super::visitor::Visitor;
use super::{
    MalformedIdentityExcerpt, Observer, PackagePolicyMembershipError as Error,
    PackagePolicyMembershipFailure, PackagePolicyMembershipLimits as Limits,
};
use crate::record::write_framed_identity;
use semantic_vocabulary::PackageKeyIdentity;

const OWNER: &str = "7f51c408406b55784a8d8130b3ddf511c938d99257c59be2ea74046ebc7bd4e2";

fn package(hexadecimal: &str) -> PackageKeyIdentity {
    let mut digest = [0; 32];
    for (slot, pair) in digest.iter_mut().zip(hexadecimal.as_bytes().chunks(2)) {
        *slot = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    PackageKeyIdentity::from_digest(digest).unwrap()
}

fn owner() -> PackageKeyIdentity {
    package(OWNER)
}

fn nominal() -> String {
    format!("nominal(package-owner(32:{OWNER}),path(Wrapper))")
}

fn framed(label: &str, children: &[&str]) -> String {
    let mut identity = String::new();
    write_framed_identity(&mut identity, label, children.iter().copied()).unwrap();
    identity
}

fn escaped(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            let reserved = matches!(character, '\\' | '(' | ')' | ',');
            reserved.then_some('\\').into_iter().chain([character])
        })
        .collect()
}

fn callable(parameter: &str) -> String {
    let parameter = framed("parameter", &[parameter, "false:false"]);
    framed(
        "conformance-callable",
        &[
            "machine",
            "overload",
            "path",
            "machine",
            &parameter,
            &framed("result-none", &[]),
        ],
    )
}

fn visitor(
    limits: Limits,
    in_source_graph: bool,
) -> Visitor<impl FnMut(PackageKeyIdentity) -> bool> {
    Visitor::new(move |_| in_source_graph, limits)
}

fn roomy() -> Limits {
    Limits::new(1 << 20, 1 << 20, 128)
}

#[test]
fn a_readable_identity_in_the_source_graph_is_accepted() {
    let mut accepting = visitor(roomy(), true);
    assert_eq!(
        accepting.type_identity(&format!("ref({})", nominal())),
        Ok(())
    );
    assert_eq!(accepting.nominal_path(&callable(&nominal())), Ok(()));
    assert_eq!(accepting.unreadable_identity(), None);
}

#[test]
fn an_owner_outside_the_source_graph_is_unknown_not_malformed() {
    let mut rejecting = visitor(roomy(), false);
    assert_eq!(
        rejecting.type_identity(&nominal()),
        Err(Error::UnknownPackage { package: owner() })
    );
    assert_eq!(rejecting.unreadable_identity(), None);
}

#[test]
fn exhausting_a_limit_is_not_reported_as_malformed() {
    let identity = format!("ref(ref(slice({})))", nominal());
    let mut shallow = visitor(Limits::new(1 << 20, 1 << 20, 2), true);
    assert_eq!(
        shallow.type_identity(&identity),
        Err(Error::NestingLimitExceeded)
    );
    let mut few_nodes = visitor(Limits::new(1 << 20, 3, 128), true);
    assert_eq!(
        few_nodes.type_identity(&identity),
        Err(Error::IdentityNodeLimitExceeded)
    );
    let decoded = format!("generic(name({}),{})", escaped(&nominal()), nominal());
    let mut few_bytes = visitor(Limits::new(8, 1 << 20, 128), true);
    assert_eq!(
        few_bytes.type_identity(&decoded),
        Err(Error::OwnedBytesLimitExceeded)
    );
    for exhausted in [&mut shallow, &mut few_nodes] {
        assert_eq!(exhausted.unreadable_identity(), None);
    }
    assert_eq!(few_bytes.unreadable_identity(), None);
}

#[test]
fn limits_inside_a_framed_name_keep_their_own_report() {
    let callable = callable(&nominal());
    let tight = Limits::new(1 << 20, 4, 128);
    assert_eq!(
        visitor(tight, true).nominal_path(&callable),
        Err(Error::IdentityNodeLimitExceeded)
    );
    let embedded = format!("generic(name({}))", escaped(&callable));
    assert_eq!(visitor(roomy(), true).type_identity(&embedded), Ok(()));
    assert_eq!(
        visitor(tight, true).type_identity(&embedded),
        Err(Error::IdentityNodeLimitExceeded)
    );
}

#[test]
fn a_signature_returning_a_generic_with_const_operands_is_readable() {
    let runtime = format!(
        r"generic(name(nominal\(toolchain-source-owner\(32:{OWNER}\)\,path\(ForeignBinding\)\)),integer(9),integer(8),integer(9))"
    );
    let topology = framed("generic", &["const-expression"; 3]);
    let signature = framed("signature-type", &[&runtime, &topology]);
    let mut accepting = visitor(roomy(), true);
    assert_eq!(accepting.type_identity(&signature), Ok(()));
    assert_eq!(accepting.type_identity(&runtime), Ok(()));
    assert_eq!(accepting.unreadable_identity(), None);
    let owned = format!(
        r"generic(name(nominal\(package-owner\(32:{OWNER}\)\,path\(Window\)\)),{},integer(4))",
        nominal()
    );
    assert_eq!(visitor(roomy(), true).type_identity(&owned), Ok(()));
    assert_eq!(
        visitor(roomy(), false).type_identity(&owned),
        Err(Error::UnknownPackage { package: owner() })
    );
}

const STD: &str = "9bc43b1779c3ad076eaf4d547d76025beac9cb005957e9c0f39cff1121519d4f";
const STD_FOREIGN_BINDING_CALLABLE: &str = r#"20:conformance-callable19:appkit_load_binding177:named-callable(path(nominal\(package-owner\(32:9bc43b1779c3ad076eaf4d547d76025beac9cb005957e9c0f39cff1121519d4f\)\,path\(appkit_load_binding\)\)),parameters(),result-dispatch())19:appkit_load_binding7:machine224:11:result-type206:14:signature-type178:generic(name(nominal\(toolchain-source-owner\(32:9ced29a4dddb319ba059ac16618d7e595ebb8d643d3a3b9f51c56245361ce613\)\,path\(ForeignBinding\)\)),integer(50),integer(18),integer(0))5:named"#;
const STD_FIXED_VEC_SIGNATURE: &str = r#"14:signature-type196:generic(name(nominal\(toolchain-source-owner\(32:6ec9951bfbb8a2f30edae4b9e6cd09fc324f6bde87f5e557df4a72997cc3fc16\)\,path\(FixedVec\)\)),named(name(type-parameter:1)),const-name(type-parameter:2))23:7:generic5:named5:named"#;
const STD_FIXED_VEC_REFERENCE_SIGNATURE: &str = r#"14:signature-type201:ref(generic(name(nominal\(toolchain-source-owner\(32:6ec9951bfbb8a2f30edae4b9e6cd09fc324f6bde87f5e557df4a72997cc3fc16\)\,path\(FixedVec\)\)),named(name(type-parameter:1)),const-name(type-parameter:2)))45:9:reference6:elided23:7:generic5:named5:named"#;

#[test]
fn std_identities_with_const_operands_are_readable() {
    let std = package(STD);
    let mut accepting = Visitor::new(move |identity| identity == std, roomy());
    assert_eq!(accepting.nominal_path(STD_FOREIGN_BINDING_CALLABLE), Ok(()));
    assert_eq!(accepting.type_identity(STD_FIXED_VEC_SIGNATURE), Ok(()));
    assert_eq!(
        accepting.type_identity(STD_FIXED_VEC_REFERENCE_SIGNATURE),
        Ok(())
    );
    assert_eq!(accepting.unreadable_identity(), None);
}

#[test]
fn a_derived_declaration_names_owners_the_source_graph_must_contain() {
    let origin = format!("generic(name({}),{})", escaped(&nominal()), nominal());
    let derived = format!("derived-application({origin})");
    let identity = format!("named(name({}))", escaped(&derived));
    assert_eq!(visitor(roomy(), true).type_identity(&identity), Ok(()));
    assert_eq!(
        visitor(roomy(), false).type_identity(&identity),
        Err(Error::UnknownPackage { package: owner() })
    );
}

#[test]
fn a_zero_package_owner_is_malformed() {
    let zero = "0".repeat(64);
    let identity = format!("nominal(package-owner(32:{zero}),path(Wrapper))");
    assert_eq!(
        visitor(roomy(), true).type_identity(&identity),
        Err(Error::MalformedIdentity)
    );
}

#[test]
fn a_rejected_identity_reports_the_text_the_grammar_stopped_on() {
    let identity = format!("ref({},elided-region)", nominal());
    let mut rejecting = visitor(roomy(), true);
    assert_eq!(
        rejecting.type_identity(&identity),
        Err(Error::MalformedIdentity)
    );
    let excerpt = rejecting
        .unreadable_identity()
        .expect("a rejected identity is kept");
    assert_eq!(excerpt.field(), "runtime type identity");
    assert_eq!(excerpt.identity_length(), identity.len());
    assert_eq!(excerpt.stopped_at(), identity.find(",elided"));
    assert_eq!(excerpt.text(), identity);
    assert_eq!(
        rejecting.unreadable_identity(),
        None,
        "an excerpt is taken once"
    );
}

#[test]
fn the_innermost_rejected_identity_is_the_one_reported() {
    let rejected = "ref(compiler-type(atom(u8)),elided-region)";
    let signature = framed("signature-type", &[rejected, "named"]);
    let mut rejecting = visitor(roomy(), true);
    assert_eq!(
        rejecting.type_identity(&signature),
        Err(Error::MalformedIdentity)
    );
    let excerpt = rejecting.unreadable_identity().unwrap();
    assert_eq!(excerpt.field(), "runtime type identity");
    assert_eq!(excerpt.text(), rejected);
}

#[test]
fn a_framing_rejection_reports_the_framed_text() {
    let truncated = framed("signature-type", &[]);
    let mut rejecting = visitor(roomy(), true);
    assert_eq!(
        rejecting.type_identity(&truncated),
        Err(Error::MalformedIdentity)
    );
    let excerpt = rejecting.unreadable_identity().unwrap();
    assert_eq!(excerpt.field(), "type identity");
    assert_eq!(excerpt.stopped_at(), None);
    assert_eq!(excerpt.text(), truncated);
}

#[test]
fn an_excerpt_is_a_bounded_window_around_the_stop() {
    let padding = "x".repeat(2_000);
    let name = format!(r"compiler-type\(atom\({padding}\)\)");
    let identity = format!("ref(generic(name({name}),elided))");
    let mut rejecting = visitor(roomy(), true);
    assert_eq!(
        rejecting.type_identity(&identity),
        Err(Error::MalformedIdentity)
    );
    let excerpt = rejecting.unreadable_identity().unwrap();
    let stopped = excerpt
        .stopped_at()
        .expect("the grammar reports where it stopped");
    assert!(excerpt.text().len() <= 256);
    assert!(identity.contains(excerpt.text()));
    assert!(excerpt.text().ends_with("elided))"));
    let rendered =
        PackagePolicyMembershipFailure::new(Error::MalformedIdentity, Some(excerpt)).to_string();
    assert!(rendered.starts_with("package policy membership: MalformedIdentity in "));
    assert!(rendered.contains(&format!("of {} bytes", identity.len())));
    assert!(rendered.contains(&format!("unreadable at byte {stopped}")));
}

#[test]
fn only_a_malformed_identity_carries_an_excerpt() {
    let excerpt = MalformedIdentityExcerpt::new("type identity", "x", None);
    let failure = PackagePolicyMembershipFailure::new(Error::NestingLimitExceeded, Some(excerpt));
    assert_eq!(failure.identity(), None);
    assert_eq!(
        failure.to_string(),
        "package policy membership: NestingLimitExceeded"
    );
}
