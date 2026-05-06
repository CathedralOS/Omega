# State And Transition Model

This note captures the current direction for Omega machines.

The short version:

- states execute code
- transitions are graph edges at the end of state bodies
- `state entry` is the implicit entry point for `machine main`
- calls execute another state or platform boundary without changing the current control-flow edge
- events and guards decide which edge is taken
- ordered state-local transitions replace inline branching

This keeps the language closer to a real state graph, which is better for proofs, debugger UX, and code generation.

## Why transitions should not run code

A transition is a handoff from one active state to another. If it also owns executable code, it starts acting like a function call, and Omega loses the clean graph shape that makes it interesting.

Bad direction:

```omega
state running {
    platform.poll_frame(...);
    run_rendering();
}
```

Better direction:

```omega
state polling_window_events {
    platform.poll_frame(main_window, mut frame_input);

    -> shutdown when frame_input.close_requested;
    -> running_game_iteration;
}
```

The state performs work. The transitions describe possible exits.

## Branch-free states

Current design preference:

States should be branch-free. There is no `if` or `else` inside normal Omega state code.

Instead, a state performs a straight-line unit of work, then reaches required trailing transition lines. The machine evaluates that state's outgoing edges in source order. The first enabled edge wins.

```omega
state polling_window_events {
    platform.poll_frame(main_window, mut frame_input);

    -> shutdown when frame_input.close_requested;
    -> running_game_iteration;
}
```

This keeps exits physically attached to the state they leave, and it gives tools a direct way to show "these are the possible exits from the current state."

The trailing transition list becomes the branch table.

## Suggested vocabulary

`machine`

Owns data, child machines, states, and transitions.

For a machine named `main`, `state entry` is the entry point. The OS process result should be modeled as owned data, such as `owns return_code: i32`, and updated through explicit mutation rather than returned from the state.

There is no `return` keyword in `state entry`. When a program exits, the root machine reaches a terminal state and performs an explicit platform handoff:

```omega
state shutdown {
    return_code = 0;
    platform.exit_process(return_code);
}
```

The return code is just data owned by the root machine. The OS boundary is an explicit platform state call.

Newer sketches are exploring typed states that can produce values through a constrained state graph. That is a separate design thread from process exit and call-style return values. See [Typed States And Invariants](typed-states-and-invariants.md).

`state`

Executable block. It may run one operation, many operations, or eventually be restricted to a smaller unit if we want highly granular debugging and proof steps.

`transition`

Declarative edge from one state to another. It has no body and does not use a `return` keyword.

Within a machine, a transition is a goto. It does not create a call frame, does not store a return address, and does not resume the state it left.

If multiple transitions leave the same state, they appear as trailing `-> target` lines and are evaluated in source order. The first enabled edge is selected. A bare `-> target` edge is unconditional.

`-> self;` is a self-transition. It re-enters the current state without repeating the state name.

A trailing bare `->` is terminal completion. It deliberately has no target and no semicolon:

```omega
state collect_key(mut inventory: Inventory) {
    inventory.has_key = true;

    ->
}
```

This says the state may complete here. If the state is running as part of nested machine flow, completion resumes the explicit continuation carried by the parent. If the state is a typed state with a value, a final expression produces the value instead of a `return` statement:

```omega
state clamp_done(value: f32) -> f32 {
    value
}
```

Nested machine flow can be sketched as two arrows:

```omega
state running {
    -> dungeon.entry -> shutdown;
}
```

This means the parent transitions into the child machine's `entry` state, and when that child reaches terminal completion (`->`), parent control resumes at `shutdown`. This avoids a special `.finished` property on every machine while keeping the continuation visible in source.

This is the stack-like exception. A parent may enter a child machine and carry an explicit continuation, but ordinary transitions inside a machine remain gotos.

Typed-state sketches may allow state signatures and return value compatibility checks. In that model, transitions are still handoffs: the target state's parameters must match, and its return value must satisfy the source graph's return value obligation, but there is no hidden caller stack inside the machine.

`call`

Callable state behavior that does explicit work without selecting the caller's next transition. Calls can target local states, contained machines, or platform boundaries. They should still make mutation visible through `mut` parameters rather than hidden ambient state.

`event`

A value or signal that can wake a dormant state or satisfy an edge.

## Transition forms

These are plausible transition forms, not final syntax.

```omega
state a {
    do_work();

    -> b when done;
    -> c when retry_count < 3;
    -> self;
}

state waiting {
    wait_for_http();

    -> complete when request.status == HttpStatus::Ok;
    -> retry when request.failed;
}

state backing_off {
    sleep();

    -> waiting when elapsed_ms >= retry_delay_ms;
}
```

Game-style weighted or event-specific transition forms may still be worth exploring later, but the active sketch keeps only `when`. That makes transition priority and proof conditions easier to explain while the language is young.

## Event-driven states

Event-driven transitions fit the model well.

```omega
state waiting_for_input {
    platform.sleep_until_event();

    -> handling_click when event == Event::MouseClick;
    -> shutdown when event == Event::WindowClose;
}
```

The important part is that the state can become dormant, and the transition edge explains what wakes it.

## HTTP request example

An HTTP request is a good example of why state/transition separation helps.

```omega
machine FetchUser {
    contains http: HttpPlatform;

    owns request: HttpRequest;
    owns retry_count: u32 = 0;
    owns retry_delay_ms: u32 = 100;

    state entry {
        http.start_request(mut request, "/user");

        -> waiting;
    }

    state waiting {
        http.poll_request(mut request);

        -> success when request.status == HttpStatus::Ok;
        -> retry when request.failed && retry_count < 3;
        -> failed when request.failed && retry_count >= 3;
        -> waiting;
    }

    state retry {
        retry_count = retry_count + 1;
        retry_delay_ms = retry_delay_ms * 2;

        -> backing_off;
    }

    state backing_off {
        http.sleep(retry_delay_ms);

        -> entry;
    }

    state success {
        // Terminal success state.
    }

    state failed {
        // Terminal failure state.
    }
}
```

This produces a graph that is obvious to inspect:

entry -> waiting -> success

waiting -> retry -> backing_off -> entry

waiting -> failed

## Proof implications

This model maps well onto TLA+ style thinking.

- Machine-owned data becomes variables.
- State execution becomes an action over variables.
- Transitions become the relation that selects the next active state.
- Guards become predicates on the transition relation.
- Transition order becomes explicit priority for enabled edges.
- Events become external inputs or fairness constraints.
- Invariants are checked over all reachable machine states.

The clean separation gives the compiler natural proof boundaries:

- prove each state preserves local invariants unless an outgoing edge permits a change
- prove every guarded transition targets a valid state
- prove terminal states are intentional
- prove liveness properties such as "every request eventually reaches Success or Failed"

## UX implications

This is also better for tooling.

A debugger can expose:

- current active state
- current source location inside the state body
- outgoing transitions and why each is enabled or disabled
- ordered outgoing transition priority
- last edge taken
- dormant states waiting on events
- state graph visualization

Breakpoints become clearer:

- break on state entry
- break on a call inside a state
- break before transition selection
- break when a specific edge is taken

## Open questions

- Should a state be allowed to contain multiple calls, or should it execute one operand and transition immediately?
- Should at least one trailing transition be mandatory on every non-terminal state?
- Is a final bare transition mandatory when the outgoing edge set is incomplete?
- Should ordered edges be the only priority mechanism, or should priority be explicit?
- How should async platform calls become events?
- Can the compiler prove that every non-terminal state has at least one possible outgoing edge?
