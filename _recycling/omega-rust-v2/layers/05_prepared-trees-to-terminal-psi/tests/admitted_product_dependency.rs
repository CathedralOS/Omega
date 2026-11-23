//! Exercises the Product compiler after Build has supplied package edges.
#[path = "sample_baseline/pipeline.rs"]
mod pipeline;

use omega_psi::{CallTarget, OperationKind, ScalarLiteral};
use omega_resolver::Dependency;

const CONSUMER: &[u8] = br"use kit\score;
use omega\language\core\ProcessExit;
machine read_score()->i32 { score() }
machine start() reaches ProcessExit { ProcessExit::exit_process(read_score()); }
";

fn compile(provider: &[u8], edges: &[Dependency<'_>]) -> Result<omega_psi::LoweredPackage, pipeline::Refusal> {
    pipeline::lower_admitted(&[("consumer", "main.omg", CONSUMER), ("provider", "main.omg", provider)], edges)
}

const KIT: Dependency<'static> = Dependency { requester: "consumer", alias: "kit", package: "provider" };

#[test]
fn admitted_provider_changes_the_called_body_in_the_product() {
    for value in [7, 9] {
        let provider = format!("public machine score()->i32 {{{value}}}");
        let product = compile(provider.as_bytes(), &[KIT]).unwrap();
        let caller = product.machines.iter().find(|m| m.declaration.package == "consumer" && m.declaration.path == "read_score").unwrap();
        let calls: Vec<_> = caller.body.operations.iter().filter_map(|op| match op.kind {
            OperationKind::Call { callee: CallTarget::Machine(id), .. } => Some(id),
            _ => None,
        }).collect();
        let [callee] = calls[..] else { panic!("expected one selected provider call, got {calls:?}") };
        let provider_body = &product.machines[(callee.get() - 1) as usize];
        assert_eq!((provider_body.declaration.package.as_str(),provider_body.declaration.path.as_str()), ("provider","score"));
        let literals: Vec<_> = provider_body.body.operations.iter().filter_map(|op| match &op.kind {
            OperationKind::Constant(literal) => Some(literal.clone()),
            _ => None,
        }).collect();
        assert_eq!(literals, [ScalarLiteral::Integer(value.into())]);
    }
}

#[test]
fn missing_registration_and_changed_exports_fail_at_their_own_layer() {
    let score = b"public machine score()->i32 {7}";
    let wrong_alias = Dependency { alias: "other", ..KIT };
    for edges in [&[][..], &[wrong_alias][..]] {
        let error = compile(score, edges).unwrap_err();
        assert_eq!((error.stage, error.package.as_str(), error.reason.as_str()), ("L2", "consumer", r"unavailable import kit\score"));
    }

    let renamed = compile(b"public machine different()->i32 {7}", &[KIT]).unwrap_err();
    assert_eq!((renamed.stage, renamed.package.as_str()), ("L2", "consumer"));
    assert!(renamed.reason.contains("score"), "{renamed:?}");

    let wrong_type = compile(b"public machine score()->bool {true}", &[KIT]).unwrap_err();
    assert_eq!((wrong_type.stage, wrong_type.package.as_str()), ("L3", "consumer"));
    assert!(wrong_type.reason.contains("expected i32, found bool"), "{wrong_type:?}");
}
