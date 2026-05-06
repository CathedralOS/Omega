# State And Transition Model

This note captures the current direction for Omega machines.

The short version:

- states execute code
- transitions are graph edges at the end of state bodies
- `state Main` is the implicit entry point for `machine main`
- commands mutate context
- queries read or fill views
- events and guards decide which edge is taken
- ordered state-local transitions replace inline branching

This keeps the language closer to a real state graph, which is better for proofs, debugger UX, and code generation.

## Why transitions should not run code

A transition is a handoff from one active state to another. If it also owns executable code, it starts acting like a function call, and Omega loses the clean graph shape that makes it interesting.

Bad direction:

```omega
state Running {
    platform.PollFrame(...);
    RunRendering();
}
```

Better direction:

```omega
state PollingWindowEvents {
    platform.PollFrame(main_window, mut frame_input);

    -> Shutdown when frame_input.close_requested;
    -> RunningGameIteration;
}
```

The state performs work. The transitions describe possible exits.

## Branch-free states

Current design preference:

States should be branch-free. There is no `if` or `else` inside normal Omega state code.

Instead, a state performs a straight-line unit of work, then reaches required trailing transition lines. The machine evaluates that state's outgoing edges in source order. The first enabled edge wins.

```omega
state PollingWindowEvents {
    platform.PollFrame(main_window, mut frame_input);

    -> Shutdown when frame_input.close_requested;
    -> RunningGameIteration;
}
```

This keeps exits physically attached to the state they leave, and it gives tools a direct way to show "these are the possible exits from the current state."

The trailing transition list becomes the branch table.

## Suggested vocabulary

`machine`

Owns data, child machines, states, and transitions.

For a machine named `main`, `state Main` is the entry point. The OS process result should be modeled as owned data, such as `owns return_code: i32`, and updated through explicit mutation rather than returned from the state.

There are no language-level returns from `state Main`. When a program exits, the root machine reaches a terminal state and performs an explicit platform handoff:

```omega
state Shutdown {
    return_code = 0;
    platform.ExitProcess(return_code);
}
```

The return code is just data owned by the root machine. The OS boundary is a command.

Newer sketches are exploring typed states that can produce values through a constrained state graph. That is a separate design thread from process exit and command-style return values. See [Typed States And Invariants](typed-states-and-invariants.md).

`state`

Executable block. It may run one operation, many operations, or eventually be restricted to a smaller unit if we want highly granular debugging and proof steps.

`transition`

Declarative edge from one state to another. It has no body and does not return values.

Within a machine, a transition is a goto. It does not create a call frame, does not store a return address, and does not resume the state it left.

If multiple transitions leave the same state, they appear as trailing `-> Target` lines and are evaluated in source order. The first enabled edge is selected. A bare `-> Target` edge is unconditional.

`-> self;` is a self-transition. It re-enters the current state without repeating the state name.

Nested machine flow can be sketched as two arrows:

```omega
state Running {
    -> dungeon.Main -> Shutdown;
}
```

This means the parent transitions into the child machine's `Main` state, and when that child reaches `-> return;`, parent control resumes at `Shutdown`. This avoids a special `.finished` property on every machine while keeping the continuation visible in source.

This is the stack-like exception. A parent may enter a child machine and carry an explicit continuation, but ordinary transitions inside a machine remain gotos.

Typed-state sketches may allow state signatures and return value compatibility checks. In that model, transitions are still handoffs: the target state's parameters must match, and its return value must satisfy the source graph's return value obligation, but there is no hidden caller stack inside the machine.

`command`

Callable behavior that mutates explicit context. This is useful for platform operations, room ticks, and other effects that are not state handoffs.

`query`

Callable behavior that reads state or writes a view buffer. Queries should not advance the machine.

`event`

A value or signal that can wake a dormant state or satisfy an edge.

## Transition forms

These are plausible transition forms, not final syntax.

```omega
state A {
    DoWork();

    -> B when done;
    -> C when retry_count < 3;
    -> self;
}

state Waiting {
    WaitForHttp();

    -> Complete when request.status == HttpStatus::Ok;
    -> Retry when request.failed;
}

state BackingOff {
    Sleep();

    -> Waiting when elapsed_ms >= retry_delay_ms;
}
```

Game-style weighted or event-specific transition forms may still be worth exploring later, but the active sketch keeps only `when`. That makes transition priority and proof conditions easier to explain while the language is young.

## Event-driven states

Event-driven transitions fit the model well.

```omega
state WaitingForInput {
    platform.SleepUntilEvent();

    -> HandlingClick when event == Event::MouseClick;
    -> Shutdown when event == Event::WindowClose;
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

    state Main {
        http.StartRequest(mut request, "/user");

        -> Waiting;
    }

    state Waiting {
        http.PollRequest(mut request);

        -> Success when request.status == HttpStatus::Ok;
        -> Retry when request.failed && retry_count < 3;
        -> Failed when request.failed && retry_count >= 3;
        -> Waiting;
    }

    state Retry {
        retry_count = retry_count + 1;
        retry_delay_ms = retry_delay_ms * 2;

        -> BackingOff;
    }

    state BackingOff {
        http.Sleep(retry_delay_ms);

        -> Main;
    }

    state Success {
        // Terminal success state.
    }

    state Failed {
        // Terminal failure state.
    }
}
```

This produces a graph that is obvious to inspect:

Main -> Waiting -> Success

Waiting -> Retry -> BackingOff -> Main

Waiting -> Failed

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
- break on a command inside a state
- break before transition selection
- break when a specific edge is taken

## Open questions

- Should a state be allowed to contain multiple commands, or should it execute one operand and transition immediately?
- Should at least one trailing transition be mandatory on every non-terminal state?
- Is a final bare transition mandatory when the outgoing edge set is incomplete?
- Should ordered edges be the only priority mechanism, or should priority be explicit?
- How should async platform calls become events?
- Can the compiler prove that every non-terminal state has at least one possible outgoing edge?
