use super::*;

const SOURCE: &str = r#"
pub data Pair { left: u64; right: u64; }
machine write_one(value: &mut u64) { value = 1; }
pub machine update(pair: &mut Pair) {
    transition { _ -> apply(pair) }
    state apply(current: &mut Pair) { write_one(&mut current.left); }
}
"#;

#[test]
fn selected_service_receiver_write_frame_rejoins_checked_source() {
    let fixture = Fixture::with_build(
        r#"
use omega::language::core::service;
pub boundary trait ClockHost { machine ticks(value: u64) -> u64; }
data Clock {}
machine Clock::ticks(value: u64) -> u64 satisfies ClockHost::ticks { value }
pub data Board { clock: Service<ClockHost> in Bound; marker: u64; other: u64; }
pub machine Board::read(&mut self) -> u64 reaches ClockHost invokes ClockHost; {
    self.marker = 1;
    self.clock.ticks(7)
}
"#,
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); builder.select_provider<ClockHost, Clock>(); }",
    );
    let machine = fixture
        .checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Board::read")
        .unwrap();
    let entry = &fixture.checked.machine_states(machine)[0];
    let fresh = validation::CallFrameResolver::new(&fixture.checked.typed)
        .unwrap()
        .inferred_state_write_frame(machine, entry);
    let resolver = validation::CallFrameResolver::new(&fixture.checked.typed).unwrap();
    for candidate in fixture.checked.machines() {
        let _ = resolver.inferred_machine_state_write_frames(candidate);
    }
    let warmed = resolver.inferred_state_write_frame(machine, entry);
    assert_eq!(
        fresh, warmed,
        "query order cannot change write-frame meaning"
    );
    let policy = project(&fixture);
    assert!(
        callable(&policy, "Board::read")
            .mutation()
            .paths()
            .contains(&"self.clock".to_owned())
    );
    let source = fixture
        .checked
        .pre_selected_dispatch_source_trees()
        .expect("verified dispatch source");
    let (call_handle, original_call) = source
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            if let typed_trees::expression::ExpressionNode::Call(call) = expression {
                (call.target.as_str() == "ticks").then_some((handle, call.clone()))
            } else {
                None
            }
        })
        .expect("original source service call");
    let argument = source
        .expression_table
        .expression_handles(original_call.arguments)[0];
    drop(source);
    let mut altered = fixture.checked.clone();
    *altered.typed.expression_table.expression_mut(argument) =
        typed_trees::expression::ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(8),
        );
    assert!(
        altered.pre_selected_dispatch_source_trees().is_err(),
        "settled argument contents must remain exact"
    );
    let mut altered = fixture.checked.clone();
    let typed_trees::expression::ExpressionNode::Call(call) =
        altered.typed.expression_table.expression_mut(call_handle)
    else {
        panic!("settled adapter call")
    };
    call.target_symbol = symbols::SymbolHandle::invalid();
    assert!(
        altered.pre_selected_dispatch_source_trees().is_err(),
        "settled target must remain exact"
    );
    let mut altered = fixture.checked.clone();
    let typed_trees::expression::ExpressionNode::Member(member) =
        altered.expression_table.expression(original_call.receiver)
    else {
        panic!("original receiver member")
    };
    let receiver = member.receiver;
    let typed_trees::expression::ExpressionNode::Name(path) =
        altered.typed.expression_table.expression_mut(receiver)
    else {
        panic!("original self root")
    };
    path.head_symbol = symbols::SymbolHandle::invalid();
    assert!(
        altered.pre_selected_dispatch_source_trees().is_err(),
        "dropped source receiver root remains guarded"
    );
    let mut altered = fixture.checked.clone();
    let target = altered
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .find_map(|statement| {
            if let typed_trees::statement::StatementNode::Assignment(assignment) = statement {
                Some(assignment.target)
            } else {
                None
            }
        })
        .expect("untouched source assignment");
    let other = altered
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Board")
        .and_then(|definition| {
            altered.data_members(definition).iter().find_map(|member| {
                if let typed_trees::data::DataMember::Field(field) = member {
                    (field.name.as_str() == "other").then_some(field.symbol)
                } else {
                    None
                }
            })
        })
        .unwrap();
    let typed_trees::expression::ExpressionNode::Member(member) =
        altered.typed.expression_table.expression_mut(target)
    else {
        panic!("assignment member")
    };
    member.member = typed_trees::name::Identifier::generated("other");
    member.member_symbol = other;
    altered
        .pre_selected_dispatch_source_trees()
        .expect("unrelated source write is not overwritten by restoration");
    assert!(
        project_checked_callable_policy(&altered, fixture.target, package_identity()).is_err(),
        "exact source-frame replay still rejects changed untouched writes"
    );
}

#[test]
fn float_and_boundary_source_views_restore_both_nesting_orders() {
    use typed_trees::expression::ExpressionNode;

    for (expression, boundary_is_outer) in [
        ("F32::negate(self.clock.ticks(7.0f32))", false),
        ("self.clock.ticks(F32::negate(7.0f32))", true),
    ] {
        let fixture = Fixture::with_build(
            &r#"
use omega::language::core::service;
pub data F32 {}
pub boundary operator F32::negate(value: f32) -> f32;
pub data FloatProvider {}
pub machine FloatProvider::negate(value: f32) -> f32
    satisfies F32::negate via Binding::CompilerIntrinsic;
pub boundary trait ClockHost { machine ticks(value: f32) -> f32; }
data Clock {}
machine Clock::ticks(value: f32) -> f32 satisfies ClockHost::ticks { value }
pub data Board { clock: Service<ClockHost> in Bound; }
pub machine Board::read(&mut self) -> f32 reaches ClockHost invokes ClockHost; {
    F32::negate(self.clock.ticks(7.0f32))
}
"#
            .replace("F32::negate(self.clock.ticks(7.0f32))", expression),
            r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_provider<F32::negate, FloatProvider>();
    builder.select_provider<ClockHost, Clock>();
}"#,
        );
        let source = fixture
            .checked
            .pre_selected_dispatch_source_trees()
            .expect("source review restores both nested dispatch edits");
        let operator = source
            .operators()
            .iter()
            .find(|operator| {
                source
                    .operator_path_members(operator.name)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(["F32", "negate"])
            })
            .expect("authored float operator");
        let uses = source
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| match expression {
                ExpressionNode::Call(call)
                    if typed_trees::operator::resolve_named_expression_call(&source, call)
                        .is_some_and(|selected| selected.symbol == operator.symbol) =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [negate] = uses.as_slice() else {
            panic!("one exact restored float call");
        };
        let negate = *negate;
        let ExpressionNode::Call(original_negate) = source.expression_table.expression(negate)
        else {
            panic!("restored named float operator");
        };
        assert_eq!(
            typed_trees::operator::resolve_named_expression_call(&source, original_negate)
                .map(|selected| selected.symbol),
            Some(operator.symbol),
        );
        assert_eq!(
            source.expression_table.expression(original_negate.receiver),
            fixture
                .checked
                .expression_table
                .expression(original_negate.receiver),
            "the named operator retains its authored namespace receiver",
        );
        let [float_operand] = source
            .expression_table
            .expression_handles(original_negate.arguments)
        else {
            panic!("one exact float operand");
        };
        let requirement = source
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "ClockHost")
            .and_then(|definition| {
                source
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.name.as_str() == "ticks")
            })
            .expect("exact authored boundary requirement");
        let calls = source
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| match expression {
                ExpressionNode::Call(call) if call.target_symbol == requirement.symbol => {
                    Some((handle, call))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(ticks, original_ticks)] = calls.as_slice() else {
            panic!("one restored call to the exact boundary requirement");
        };
        assert_eq!(original_ticks.target.as_str(), "ticks");
        let receivers = fixture
            .checked
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| match expression {
                ExpressionNode::Member(member) if member.member.as_str() == "clock" => {
                    Some((handle, expression))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(receiver, receiver_expression)] = receivers.as_slice() else {
            panic!("one retained authored service receiver");
        };
        assert_eq!(original_ticks.receiver, *receiver);
        assert_eq!(
            source.expression_table.expression(original_ticks.receiver),
            *receiver_expression,
            "restoration preserves the exact receiver graph and symbols",
        );
        let [boundary_operand] = source
            .expression_table
            .expression_handles(original_ticks.arguments)
        else {
            panic!("one scalar boundary argument");
        };
        let literal = if boundary_is_outer {
            assert_eq!(*boundary_operand, negate);
            *float_operand
        } else {
            assert_eq!(*float_operand, *ticks);
            *boundary_operand
        };
        let ExpressionNode::Float(original_literal) = source.expression_table.expression(literal)
        else {
            panic!("nested operand restores the authored float literal");
        };
        assert_eq!(original_literal.text(), "7.0");
        assert_eq!(
            original_literal.landing(),
            Some(numerics::literals::FloatFormat::F32),
        );
        // Each journal validates its nested graph before reverse restoration;
        // neither rewritten node is treated as authored source.
        let ExpressionNode::Binary(settled_negate) =
            fixture.checked.expression_table.expression(negate)
        else {
            panic!("float settlement must replace the named operation");
        };
        assert_eq!(settled_negate.left, *float_operand);
        let ExpressionNode::Call(settled_ticks) =
            fixture.checked.expression_table.expression(*ticks)
        else {
            panic!("boundary settlement must retain its call node");
        };
        assert_eq!(settled_ticks.target.as_str(), "Clock::ticks");
        assert_ne!(settled_ticks.target_symbol, original_ticks.target_symbol);
        assert!(!settled_ticks.receiver.is_valid());
        assert_eq!(
            fixture
                .checked
                .expression_table
                .expression_handles(settled_ticks.arguments),
            &[*boundary_operand],
            "boundary settlement preserves its exact operand handle",
        );
        drop(source);
        project(&fixture);

        let mut altered = fixture.checked.clone();
        *altered.typed.expression_table.expression_mut(literal) = ExpressionNode::Float(
            numerics::literals::FloatLiteral::from_f64(8.0)
                .with_landing(numerics::literals::FloatFormat::F32),
        );
        assert!(
            altered.pre_selected_dispatch_source_trees().is_err(),
            "nested operand changes invalidate the restored source graph: {expression}"
        );
    }
}

#[test]
fn private_helper_and_state_renames_preserve_entry_root_write_meaning() {
    let original = project(&Fixture::local(SOURCE));
    let renamed_source = SOURCE
        .replace("write_one", "assign_value")
        .replace("apply", "perform")
        .replace("current", "destination");
    let renamed = project(&Fixture::local(&renamed_source));
    assert_eq!(original, renamed);
    assert_eq!(
        original.canonical_bytes().unwrap(),
        renamed.canonical_bytes().unwrap()
    );
    assert!(!callable(&original, "update").mutation().paths().is_empty());

    let redirected = project(&Fixture::local(
        &SOURCE.replace("current.left", "current.right"),
    ));
    assert_eq!(
        callable(&original, "update").parameters(),
        callable(&redirected, "update").parameters()
    );
    assert_ne!(
        callable(&original, "update").mutation(),
        callable(&redirected, "update").mutation()
    );
    assert_ne!(
        original.canonical_bytes().unwrap(),
        redirected.canonical_bytes().unwrap()
    );
}
