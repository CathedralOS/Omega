//! Fixtures shared by the unit cleanup tests: the nominal cleanup sources.

#[path = "unit_cleanup/contextual_and_multi_root_cleanups.rs"]
mod contextual_and_multi_root_cleanups;

const SCALAR_SOURCE: &str = r#"
    data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const FINITE_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; audited: bool; armed: bool; }
    machine Token::drop(&mut self)
    requires
        self.armed;
        self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires
        token.armed;
        token.audited;
        token.ready
    {}
"#;

#[test]
fn nominal_cleanup_selection_retains_named_and_legacy_borrowed_subjects() {
    for (subject, receiver) in [("token: &mut Token", "token"), ("&mut self", "self")] {
        let source = format!(
            r#"
            data Helper {{}}
            machine Helper::note(value: bool) {{}}
            data Token {{ ready: bool; }}
            machine Token::drop({subject}) {{
                Helper::note({receiver}.ready);
                {receiver}.ready = false;
            }}
            data Root {{}}
            machine Root::enter(token: Token) {{}}
            "#
        );
        let checked = crate::front_end::checked_program(&source);
        let root = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .expect("authored consuming caller");
        let nominal = checked
            .facts
            .flow
            .terminal_nominal_affine_unit_cleanups
            .machines
            .iter()
            .find(|plan| plan.machine.machine == root.symbol)
            .unwrap_or_else(|| panic!("the {subject} hook must select a nominal cleanup plan"));
        let [cleanup] = nominal.cleanups.as_slice() else {
            panic!("one owned argument has one selected cleanup action")
        };
        let hook = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|machine| machine.machine == cleanup.cleanup_machine)
            .expect("selected hook retains its ordinary checked body");
        let [parameter] = hook.structural_parameters.as_slice() else {
            panic!("the hook body reads and writes its single borrowed subject")
        };
        assert_eq!(parameter.type_identity, cleanup.type_identity);
        assert_eq!(parameter.position, 0);
        assert_eq!(
            parameter.access,
            checked_trees::CheckedStructuralAccess::MutableBorrow,
        );
        assert!(hook.operations.iter().any(|operation| matches!(
            operation,
            checked_trees::CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
        )));
    }
}
