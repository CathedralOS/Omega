use super::*;

fn application(owner: TraitDefinition) -> Application {
    let lifetime_arguments = vec![Identifier::generated("x"); owner.lifetime_parameters.len()];
    Application {
        owner,
        arguments: Vec::new(),
        lifetime_arguments,
        inherited_substitutions: Vec::new(),
    }
}

#[test]
fn repeated_trait_lifetime_binder_rejects() {
    let mut compilation = TypedTrees::default();
    let owner = TraitDefinition {
        lifetime_parameters: vec!["a".into(), "a".into()],
        ..TraitDefinition::default()
    };
    let error = collect(
        &mut compilation,
        application(owner),
        SymbolHandle::invalid(),
        &mut Vec::new(),
        &mut Vec::new(),
        &[],
    )
    .expect_err("a repeated declaring-trait binder must not silently collide");
    assert!(
        error[0].message.contains("repeats a trait lifetime binder"),
        "unexpected diagnostic: {}",
        error[0].message
    );
}

#[test]
fn distinct_trait_lifetime_binders_still_collect() {
    let mut compilation = TypedTrees::default();
    let owner = TraitDefinition {
        lifetime_parameters: vec!["a".into(), "b".into()],
        ..TraitDefinition::default()
    };
    collect(
        &mut compilation,
        application(owner),
        SymbolHandle::invalid(),
        &mut Vec::new(),
        &mut Vec::new(),
        &[],
    )
    .expect("distinct binders must still walk the chain");
}
