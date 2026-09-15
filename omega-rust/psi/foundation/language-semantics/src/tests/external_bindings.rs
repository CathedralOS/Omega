use crate::{ExternalBindingId, ExternalBindingIdentity, ExternalBindingTable};

#[test]
fn external_binding_interner_uses_structural_identity() {
    let mut bindings = ExternalBindingTable::default();
    let first = bindings.intern(ExternalBindingIdentity::Import {
        library: "a,b".to_owned(),
        symbol: "c".to_owned(),
    });
    assert_eq!(
        bindings.intern(ExternalBindingIdentity::Import {
            library: "a,b".to_owned(),
            symbol: "c".to_owned(),
        }),
        first,
        "equal structural bindings must share one identity"
    );
    assert_ne!(
        bindings.intern(ExternalBindingIdentity::Import {
            library: "a".to_owned(),
            symbol: "b,c".to_owned(),
        }),
        first,
        "field boundaries must remain identity-bearing"
    );
    let intrinsic = bindings.intern(ExternalBindingIdentity::CompilerIntrinsic);
    assert_ne!(
        intrinsic, first,
        "mechanism tags must remain identity-bearing"
    );
    assert_eq!(
        bindings.intern(ExternalBindingIdentity::CompilerIntrinsic),
        intrinsic,
        "payloadless intrinsic values must share one structural binding identity"
    );
    assert_eq!(
        bindings.identity(first),
        Some(&ExternalBindingIdentity::Import {
            library: "a,b".to_owned(),
            symbol: "c".to_owned(),
        })
    );
    assert_eq!(bindings.identity(ExternalBindingId(0)), None);
    assert_eq!(bindings.identity(ExternalBindingId(u32::MAX)), None);
}
