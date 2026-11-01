//! Array-expression identities must visit callable owners, never hide them in
//! opaque length text. Framing remains independent of source admission.
use crate::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
};

#[derive(Default)]
struct Owners {
    allowed: Option<[u8; 32]>,
    seen: Vec<[u8; 32]>,
    depth: usize,
}
impl TypeIdentityPackageOwnerVisitor for Owners {
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
        if self.depth == 32 {
            return Err(TypeIdentityVisitError::ResourceLimitExceeded);
        }
        self.depth += 1;
        Ok(())
    }
    fn leave(&mut self) {
        self.depth -= 1;
    }
    fn reserve(&mut self, _: usize) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
    fn package_owner(&mut self, owner: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
        self.seen.push(owner);
        if self.allowed == Some(owner) {
            Ok(())
        } else {
            Err(TypeIdentityVisitError::UnknownPackage)
        }
    }
    fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
}
fn call_extent() -> String {
    let nominal = format!("nominal(package-owner(32:{}),path(size))", "ab".repeat(32));
    let escaped = nominal
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace(',', "\\,");
    format!("array(compiler-type(atom(u8)),const-expression(call(target({escaped}),integer(3))))")
}
#[test]
fn extent_call_visits_exact_package_owner_and_refuses_unknown_owner() {
    let mut owners = Owners {
        allowed: Some([0xab; 32]),
        ..Default::default()
    };
    assert_eq!(
        visit_type_identity_package_owners(&call_extent(), &mut owners),
        Ok(())
    );
    assert_eq!(owners.seen, vec![[0xab; 32]]);
    assert_eq!(owners.depth, 0);
    let mut unknown = Owners::default();
    assert_eq!(
        visit_type_identity_package_owners(&call_extent(), &mut unknown),
        Err(TypeIdentityVisitError::UnknownPackage)
    );
    assert_eq!(unknown.depth, 0);
}
#[test]
fn extent_cast_visits_target_owner_and_rejects_unknown_policy() {
    let identity = format!(
        "array(compiler-type(atom(u8)),const-expression(cast(integer(3),nominal(package-owner(32:{}),path(Count)),domain(Wrapping))))",
        "ab".repeat(32)
    );
    let mut owners = Owners {
        allowed: Some([0xab; 32]),
        ..Default::default()
    };
    assert_eq!(
        visit_type_identity_package_owners(&identity, &mut owners),
        Ok(())
    );
    assert_eq!(owners.seen, vec![[0xab; 32]]);
    assert_eq!(
        visit_type_identity_package_owners(&identity.replace("Wrapping", "Invented"), &mut owners),
        Err(TypeIdentityVisitError::MalformedIdentity)
    );
    assert_eq!(owners.depth, 0);
}

#[test]
fn extent_static_operands_visit_nested_type_and_selection_owners() {
    let nominal = format!(
        "nominal(package-owner(32:{}),path(Selected))",
        "ab".repeat(32)
    );
    let escaped = nominal
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace(',', "\\,");
    let identity = format!(
        "array(compiler-type(atom(u8)),const-expression(call(target(count),static-arguments({nominal},static-application(const-name({escaped}),integer(7))),integer(3))))"
    );
    let mut owners = Owners {
        allowed: Some([0xab; 32]),
        ..Default::default()
    };
    assert_eq!(
        visit_type_identity_package_owners(&identity, &mut owners),
        Ok(())
    );
    assert_eq!(owners.seen, vec![[0xab; 32], [0xab; 32]]);
    assert_eq!(owners.depth, 0);
    let mut unknown = Owners::default();
    assert_eq!(
        visit_type_identity_package_owners(&identity, &mut unknown),
        Err(TypeIdentityVisitError::UnknownPackage)
    );
    assert_eq!(unknown.depth, 0);
    assert_eq!(
        visit_type_identity_package_owners(
            &identity.replace("integer(7)", "hidden-owner(7)"),
            &mut owners
        ),
        Err(TypeIdentityVisitError::MalformedIdentity)
    );
}

#[test]
fn generic_const_arguments_visit_nested_owners_and_preserve_rejections() {
    let nominal = format!("nominal(package-owner(32:{}),path(count))", "ab".repeat(32));
    let escaped = nominal
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace(',', "\\,");
    let identity = format!(
        "generic(name(Buffer),integer(50),named(name(Element)),call(target({escaped}),integer(3)))"
    );
    let mut owners = Owners {
        allowed: Some([0xab; 32]),
        ..Default::default()
    };
    assert_eq!(
        visit_type_identity_package_owners(&identity, &mut owners),
        Ok(())
    );
    assert_eq!(owners.seen, vec![[0xab; 32]]);
    assert_eq!(owners.depth, 0);
    let mut unknown = Owners::default();
    assert_eq!(
        visit_type_identity_package_owners(&identity, &mut unknown),
        Err(TypeIdentityVisitError::UnknownPackage)
    );
    assert_eq!(unknown.depth, 0);
    for malformed in [
        identity.replace("integer(3)", "hidden-owner(3)"),
        format!("{identity})"),
    ] {
        assert_eq!(
            visit_type_identity_package_owners(&malformed, &mut owners),
            Err(TypeIdentityVisitError::MalformedIdentity)
        );
        assert_eq!(owners.depth, 0);
    }
    let nested = format!(
        "{}integer(1){}",
        "generic(name(Buffer),".repeat(40),
        ")".repeat(40)
    );
    assert_eq!(
        visit_type_identity_package_owners(&nested, &mut owners),
        Err(TypeIdentityVisitError::ResourceLimitExceeded)
    );
    assert_eq!(owners.depth, 0);
}
