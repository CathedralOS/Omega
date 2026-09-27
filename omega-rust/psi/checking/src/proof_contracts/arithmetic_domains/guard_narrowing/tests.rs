use super::{
    Interval, TypedTrees, ValueEnvironment, fall_through_narrowed_environment,
    guard_narrowed_environment, incoming_guard_environment, incoming_guard_environments,
};
use typed_trees::statement::StatementNode;

mod equality_meaning;
mod receiver_returns;

#[test]
fn a_strict_place_ceiling_proves_the_increment_for_narrower_carriers() {
    // ch12's `Counter::bump` relation: `requires self.count < self.cap`
    // records `cap >= count + 1`, and `cap`'s own declared carrier bounds the
    // result by its representable high -- the composed ceiling discharge the
    // u64 lane admits through literal ceilings extends to every fixed-width
    // integer. A non-strict bound leaves no gap, a bound that does not name
    // the incremented operand proves nothing, and a ceiling whose carrier is
    // wider than the result cannot bound it.
    for (source, accepted) in [
        (
            "data Counter { count: u32; cap: u32; }
             machine Counter::bump(&mut self)
                 requires self.count < self.cap
             { self.count += 1; }",
            true,
        ),
        (
            "data Counter { count: u32; cap: u32 [0..=100]; }
             machine Counter::bump(&mut self)
                 requires self.count < self.cap
             { self.count += 1; }",
            true,
        ),
        (
            "data Counter { count: u8; cap: u8; }
             machine Counter::bump(&mut self)
                 requires self.count < self.cap
             { self.count += 1; }",
            true,
        ),
        // `count < cap` is only a gap of 1; `count + 2` could still overflow.
        (
            "data Counter { count: u32; cap: u32; }
             machine Counter::bump(&mut self)
                 requires self.count < self.cap
             { self.count += 2; }",
            false,
        ),
        // A two-hop chain `count < mid < cap` composes gap 2 through places.
        (
            "data Counter { count: u32; mid: u32; cap: u32; }
             machine Counter::bump(&mut self)
                 requires
                     self.count < self.mid
                     self.mid < self.cap
             { self.count += 2; }",
            true,
        ),
        (
            "data Counter { count: u32; cap: u32; }
             machine Counter::bump(&mut self)
                 requires self.count <= self.cap
             { self.count += 1; }",
            false,
        ),
        (
            "data Counter { count: u32; cap: u32; }
             machine Counter::bump(&mut self)
                 requires self.cap < self.count
             { self.count += 1; }",
            false,
        ),
        (
            "data Counter { count: u32; cap: u32; }
             machine Counter::bump(&mut self)
             { self.count += 1; }",
            false,
        ),
        (
            "data Counter { count: u32; cap: u64; }
             machine Counter::bump(&mut self)
                 requires self.count < self.cap
             { self.count += 1; }",
            false,
        ),
    ] {
        let program = crate::front_end::typed_program(source);
        let result = crate::validate_program(&program);
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
    }
}

#[test]
fn a_live_unsigned_ceiling_proves_joint_addition_fits_its_carrier() {
    let source = "machine sum(left: u64, right: u64, capacity: u64) -> u64
        requires right <= capacity;
        { transition left <= capacity - right {
            true -> (left + right)
            false -> 0
        } }";
    for source in [
        source.to_owned(),
        source.replace("left <= capacity - right", "capacity - right >= left"),
    ] {
        let result = crate::validate_program(&crate::front_end::typed_program(&source));
        assert!(result.is_ok(), "{source}: {result:?}");
    }
    for source in [
        source.replace("requires right <= capacity;", ""),
        source.replace("true -> (left + right)", "true -> (left + right + 1)"),
        source.replace("left <= capacity - right", "left >= capacity - right"),
    ] {
        let result = crate::validate_program(&crate::front_end::typed_program(&source));
        assert!(result.is_err(), "{source}");
    }
}

fn delivered_bounds(source: &str) -> Option<Interval> {
    let program = crate::front_end::typed_program(source);
    let machine = &program.machines()[0];
    let frames = crate::CallFrameResolver::new(&program);
    let batch = incoming_guard_environments(&program, machine, frames.as_ref());
    assert_eq!(batch.len(), program.machine_states(machine).len());
    for (state, (symbol, environment)) in program.machine_states(machine).iter().zip(&batch) {
        assert_eq!(*symbol, state.symbol);
        assert_eq!(
            *environment,
            incoming_guard_environment(&program, machine, state, frames.as_ref()),
            "batched arrivals preserve each individual query: {source}",
        );
    }
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "append")
        .unwrap();
    incoming_guard_environment(&program, machine, state, frames.as_ref()).get("delivered")
}

#[test]
fn cyclic_dependent_arrival_bounds_stay_within_the_declared_range() {
    // A `delivered + 1` delivery in a dependent-bounded cycle seeds the
    // target parameter's environment intersected with its declared range:
    // the transition delivery already proves the argument fits. Without
    // that clamp the joined arrival widened past `self.max` on every
    // fixpoint round -- a guard `x + 1 <= self.max` cannot narrow the sum
    // against the field bound -- and reads then trusted a value the
    // parameter can never hold (heat_grid's `y * 4 + x` hoist temps were
    // rejected with an environment interval of (0, 23) against their synthesized
    // `[0..=11]`).
    assert_eq!(
        delivered_bounds(
            "data Limits { max: u32 [0..=4]; }
    machine Limits::main(&mut self) {
        transition { _ -> append(0) }
        state append(&mut self, delivered: u32 [0..=self.max]) {
            transition delivered + 1 <= self.max { true -> append(delivered + 1) _ -> done() }
        }
        state done(&mut self) { crash Trap; }
    }"
        ),
        Some(Interval {
            low: Some(0),
            high: Some(4)
        })
    );
}

#[test]
fn arrival_bounds_include_statement_and_value_calls() {
    for call in ["append(value);", "let returned: u32 = append(value);"] {
        assert_eq!(
            delivered_bounds(&format!(
                "machine main(value: u32 [0..=4]) -> u32 {{
            {call}
            transition value < 4 {{ true -> append(value) false -> 0u32 }}
            state append(delivered: u32 [0..=4]) -> u32 {{ delivered }}
        }}"
            )),
            Some(Interval {
                low: Some(0),
                high: Some(4)
            }),
            "{call}"
        );
    }
}

fn program_with_guard(condition: &str) -> TypedTrees {
    let source = format!(
        "machine value(remaining: u32) -> u32 {{
             transition {condition} {{ true -> 1u32 false -> 0u32 }}
         }}"
    );
    crate::front_end::typed_program(&source)
}

#[test]
fn nested_boolean_guard_wrappers_preserve_integer_bound_polarity() {
    for (condition, positive_rank) in [
        ("remaining > 0", true),
        ("!(remaining > 0)", false),
        ("!!(remaining > 0)", true),
        ("(!(remaining > 0)) == false", true),
        ("false == (!(remaining > 0))", true),
        ("(!(remaining > 0)) != true", true),
        ("true != (!(remaining > 0))", true),
        ("!((remaining > 0) != false)", false),
    ] {
        let program = program_with_guard(condition);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("authored transition");
        };
        let selected = guard_narrowed_environment(
            &program,
            machine,
            Some(state),
            &transition.guard,
            &ValueEnvironment::new(),
        );
        let fallback = fall_through_narrowed_environment(
            &program,
            machine,
            Some(state),
            &transition.guard,
            &ValueEnvironment::new(),
        );
        let positive = Interval {
            low: Some(1),
            high: Some(i64::from(u32::MAX)),
        };
        let zero = Interval::constant(0);
        assert_eq!(
            selected.get("remaining"),
            Some(if positive_rank { positive } else { zero }),
            "{condition}: selected"
        );
        assert_eq!(
            fallback.get("remaining"),
            Some(if positive_rank { zero } else { positive }),
            "{condition}: fallback"
        );
    }
}
