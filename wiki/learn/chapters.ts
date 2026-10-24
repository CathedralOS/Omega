// Chapter content for Learn Omega. Every code block is real Omega, checked
// against the reference compiler (`omega --check` on each fixture under
// scratch/verify/). No `self` anywhere: Omega has no implicit receiver;
// context and data are passed as explicit parameters.

export interface Note {
  for?: number;        // 1-based code line the marginalia key ties to
  text: string;
}

export interface Staff {
  title?: string;
  figure:
    | { kind: "code"; code: string; highlight?: number[]; verify?: boolean; filename?: string; companion?: string }
    | { kind: "table"; head: string[]; rows: string[][] }
    | { kind: "prose"; text: string };
  notes: Note[];
}

export interface Chapter {
  id: string;
  num: string;
  title: string;
  standfirst: string;
  staves: Staff[];
}

export const chapters: Chapter[] = [
  {
    id: "ch-1",
    num: "I",
    title: "A Program Is a Machine",
    standfirst: "An Omega program has no `main` function and no ambient OS access. It starts in a machine that declares which outside services it reaches.",
    staves: [
      {
        title: "The smallest whole program",
        figure: {
          kind: "code",
          code: `use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

machine start() reaches Console + ProcessExit {
    Console::write_line("Hello, Omega.");
    ProcessExit::exit_process(0);
}`,
          highlight: [5],
        },
        notes: [
          { for: 1, text: "Process exit is its own boundary service, `ProcessExit`, separate from printing. The first two `use` lines bring in the contract and its standard-library binding." },
          { for: 3, text: "Brings in the `Console` boundary trait. `build.omg` selects the platform provider that answers it." },
          { for: 5, text: "`machine` declares a state machine with no implicit state or `self`. `reaches Console + ProcessExit` is its service ceiling: without that clause, it cannot touch the outside world." },
          { for: 6, text: "`Console::write_line` calls the boundary contract statically. The source never names a platform handle or syscall." },
          { for: 7, text: "`ProcessExit::exit_process(0)` terminates the process domain with status 0. Printing never implies permission to exit, so the signature names both services." },
        ],
      },
      {
        title: "What the supervisor reads",
        figure: {
          kind: "prose",
          text: "Review agent-written Omega in three passes: first the `reaches` clause (what outside services it can touch), then the `data` declarations (what memory it owns and its bounds), and finally the state graph (how control moves). If those three are honest, the implementation inside each state is boxed in by the checker.",
        },
        notes: [
          { text: "Capabilities first, data layout second, state transitions third. Every review in this guide follows that order." },
        ],
      },
    ],
  },
  {
    id: "ch-2",
    num: "II",
    title: "States, Not Ifs",
    standfirst: "Omega has no `if`, `while`, or `for` statements. Branching and looping are explicit state transitions without hidden call frames.",
    staves: [
      {
        title: "A guard picks the next state",
        figure: {
          kind: "code",
          code: `machine choose(flag: bool) -> u64 {
    transition flag {
        true  -> yes()
        _     -> no()
    }
    state yes() -> u64 { 70u64 }
    state no()  -> u64 { 71u64 }
}

machine main() -> u64 { choose(true) }`,
          highlight: [2, 3, 4],
        },
        notes: [
          { for: 2, text: "`transition` evaluates its subject once and jumps to the matching state. Internal state jumps transfer control directly without pushing a call frame." },
          { for: 4, text: "`_` covers every remaining value. Transition arms must be exhaustive so every path has a proven destination." },
          { for: 6, text: "A `state` is a named control point inside the machine with its own parameter frontier, not a closure that captures surrounding locals." },
          { for: 10, text: "A machine that holds no persistent state needs no `data` declaration. Its inputs and return type are its entire interface." },
        ],
      },
      {
        title: "A state that jumps to itself is a loop",
        figure: {
          kind: "code",
          code: `machine countdown(n: u64) -> u64 {
    transition n > 0u64 {
        true  -> tick(n)
        _     -> n
    }
    state tick(n: u64) -> u64 {
        let next: u64 = n - 1;
        transition next > 0u64 {
            true  -> tick(next)
            _     -> next
        }
    }
}

machine main() -> u64 { countdown(5u64) }`,
          highlight: [9],
        },
        notes: [
          { for: 7, text: "Unsigned `n - 1` is legal here because the incoming guard proved `n > 0` before jumping to `tick`. Remove the guard and exact-arithmetic checking rejects the subtraction." },
          { for: 8, text: "A state only sees its own parameters and the facts carried across its incoming edge, so `tick` tests `next > 0` explicitly before looping." },
          { for: 9, text: "`tick(next)` jumps back to `tick` with a smaller argument. Cycles in the state graph replace `while` and `for` loops." },
          { text: "When reviewing a loop, check two things: the guard that proves the step cannot underflow, and the value that strictly decreases on every back-edge." },
        ],
      },
      {
        title: "Reading the graph",
        figure: {
          kind: "prose",
          text: "Ask the agent for the state list and transitions before asking for code. Good state names describe stages of the domain workflow (`scan`, `wipe`, `done`); vague names (`step2`, `helper`, `process`) mean the control flow has not been designed yet.",
        },
        notes: [
          { text: "State names are the cheapest design review you can run, and the fastest way to catch a muddled machine." },
        ],
      },
    ],
  },
  {
    id: "ch-3",
    num: "III",
    title: "Fields Are the Memory",
    standfirst: "Machines do not own hidden fields. Persistent state lives in explicit `data` declarations with fixed layouts and is passed in by value or reference.",
    staves: [
      {
        title: "The data block is the data model",
        figure: {
          kind: "code",
          code: `data Room {
    label: [u8; 4];
}

data Level {
    rooms: [Room; 2];
    ticks: i32 in Wrapping;
}

machine rename(level: &mut Level) {
    level.rooms[1].label[0] = 82;   // 'R'
    level.ticks = level.ticks + 1;
}

machine main() {
    let mut level: Level = Level {
        rooms: [
            Room { label: "room" },
            Room { label: "hall" }
        ],
        ticks: 0,
    };
    rename(&mut level);
}`,
          highlight: [5, 6, 7, 8],
        },
        notes: [
          { for: 2, text: "`[u8; 4]` is a fixed-length array of four bytes stored inline. Capacity is part of the type, not a runtime allocation." },
          { for: 6, text: "`[Room; 2]` nests two `Room` records contiguously inside `Level`. Every byte of `Level` is accounted for at compile time." },
          { for: 7, text: "`i32 in Wrapping` qualifies the field with modular two's-complement arithmetic, allowing `level.ticks + 1` without an upper-bound proof." },
          { for: 10, text: "`&mut Level` lends mutable access to `level` for the duration of the call. There is no implicit `self` parameter." },
          { for: 11, text: "Constant indices `1` and `0` are statically in bounds against `[Room; 2]` and `[u8; 4]`, so the checker discharges both array bounds automatically." },
          { for: 16, text: "Record construction initializes every field explicitly. `let mut` permits mutation, and `&mut level` borrows the storage in place." },
          { text: "If an agent starts threading five loose parameters through every state, stop and have it group that state into a `data` record with explicit field types." },
        ],
      },
    ],
  },
  {
    id: "ch-cases",
    num: "IV",
    title: "Choice Is a Value Too",
    standfirst: "Sum types are `data` declarations with `case` members. You inspect them through exhaustive `transition` dispatch, and payloads can carry their own `where` invariants.",
    staves: [
      {
        title: "Three cases, one value",
        figure: {
          kind: "code",
          code: `data Phase [copy] {
    case Red;
    case Green;
    case Yellow;
}

data Light [copy] {
    phase: Phase;
    advances: i32 in Wrapping;
}

machine next(phase: Phase) -> Phase {
    transition {
        phase == Phase::Red    -> Phase::Green
        phase == Phase::Green  -> Phase::Yellow
        _                      -> Phase::Red
    }
}

machine advance(light: Light) -> Light {
    Light {
        phase: next(light.phase),
        advances: light.advances + 1,
    }
}

machine main() {
    let light: Light = Light {
        phase: Phase::Red,
        advances: 0,
    };
    let once: Light = advance(light);
    let twice: Light = advance(once);
    let again: Light = advance(light);
}`,
          highlight: [1, 12],
        },
        notes: [
          { for: 1, text: "Owned data is affine by default (move at most once). Marking a pure value type `[copy]` makes it unrestricted so passing it does not consume the binding." },
          { for: 2, text: "`case Red;` declares one variant of `Phase`. A `Phase` value is always one of its declared cases." },
          { for: 13, text: "Guard-list `transition { ... }` tests boolean conditions in order and evaluates only the chosen arm." },
          { for: 16, text: "The `_` fallback completes exhaustive coverage and returns `Phase::Red` when `phase` is `Yellow`." },
          { for: 21, text: "Omega has no struct-update shorthand (`..old`), so `advance` constructs the new `Light` with every field named." },
          { for: 23, text: "Because `advances` carries `i32 in Wrapping`, the addition uses modular wrapping semantics automatically." },
          { for: 34, text: "`light` was already passed on line 32, and `[copy]` lets line 34 use it again. Without `[copy]`, the checker rejects the second use because a moved affine binding holds no value." },
          { text: "Value-position `match` is only for comparing plain scalar values. When branching on case shapes or extracting payloads, use `transition` dispatch." },
        ],
      },
      {
        title: "Cases carry cargo",
        figure: {
          kind: "code",
          code: `data Reply {
    case Hit(value: i32) where value in 0..=255;
    case Miss;
}

machine read(reply: Reply) -> i32 {
    transition reply {
        Reply::Hit { value } -> keep(value)
        Reply::Miss          -> miss()
    }
    state keep(value: i32) -> i32 { value }
    state miss() -> i32 { 0 }
}

machine main() -> i32 {
    let reply: Reply = Reply::Hit { value: 66 };
    read(reply)
}`,
          highlight: [2, 7],
        },
        notes: [
          { for: 2, text: "A case-local `where` clause attaches an invariant to that variant's payload. Constructing `Reply::Hit` requires proving `value in 0..=255`; matching `Hit` recovers that fact." },
          { for: 6, text: "Subject-form `transition reply` dispatches on the active case of `reply`. Every declared case must be handled." },
          { for: 7, text: "`Reply::Hit { value }` destructures the payload by field name and passes `value` into `keep(value)` along with its `0..=255` range fact." },
          { for: 8, text: "`Reply::Miss` has no payload fields, so the variant path alone matches it." },
          { for: 16, text: "Case construction names the payload field explicitly (`Reply::Hit { value: 66 }`), and the literal `66` satisfies `0..=255` at compile time." },
          { text: "Standard-library `ByteRead` uses this exact pattern: `case Eof;` and `case Byte(value: i32) where value >= 0 && value <= 255;`." },
        ],
      },
    ],
  },
  {
    id: "ch-4",
    num: "V",
    title: "Writing for the Prover",
    standfirst: "Default integer arithmetic is Exact: every operation, index, and narrowing cast is a proof obligation. If the checker cannot prove a step fits, compilation stops.",
    staves: [
      {
        title: "Contracts are part of the signature",
        figure: {
          kind: "code",
          code: `machine bounded(input: u16) -> u16
requires input <= 255u16
ensures  result <= input
{
    transition { _ -> finish(input) }
    state finish(saved: u16) -> u16 { saved }
}`,
          highlight: [2, 3],
        },
        notes: [
          { for: 2, text: "`requires` states a precondition the caller must establish before invoking `bounded`. It is checked statically at the call site, never turned into a runtime assert." },
          { for: 3, text: "`ensures` states the postcondition guaranteed on normal return, using the reserved name `result` for the returned value." },
          { for: 5, text: "Flow facts travel across state transitions: `finish` receives `saved` knowing it equals `input`, which discharges `result <= input`." },
          { text: "When the prover refuses an expression, do not fight the syntax. Tighten a precondition, hoist an intermediate value into a guarded transition, or declare an explicit overflow policy." },
        ],
      },
      {
        title: "An empty body is a claim",
        figure: {
          kind: "code",
          code: `machine less_than_transitive(a: u64, b: u64, c: u64)
requires
    a < b
    b < c
ensures
    a < c
{
}`,
          highlight: [7, 8],
        },
        notes: [
          { for: 3, text: "Multiple `requires` rows combine as premises that hold on entry." },
          { for: 8, text: "A machine with an empty body is a pure compile-time lemma: the checker must derive `a < c` directly from `a < b` and `b < c`." },
          { text: "`samples/cli/proofs/math_proofs` builds an entire chain of arithmetic lemmas this way, without emitting a single runtime instruction." },
        ],
      },
      {
        title: "What a refusal reads like",
        figure: {
          kind: "code",
          verify: false,
          code: `error: exact arithmetic in machine \`rename\`
  state \`entry\` assignment may overflow \`i32\`:
  the operands are not provably in range
  (decision 17 -- exact arithmetic is
  a proof obligation).
  Widen with an \`as\` cast, constrain the range
  with \`requires\` or a guard, or opt into
  \`i32 in Wrapping\`/\`Saturating\`/\`Trapping\`.`,
          highlight: [4, 5],
        },
        notes: [
          { for: 1, text: "Verbatim `omega --check` diagnostic when `in Wrapping` is removed from `ticks` in Chapter III." },
          { for: 2, text: "`may overflow` means the range proof is missing, not that a specific input overflowed. Unproved exact math is rejected." },
          { for: 4, text: "Diagnostics cite the language rule (`decision 17`) so you and the agent can look up the exact contract in `wiki/spec/`." },
          { for: 6, text: "Every refusal lists the legal repairs: widen with `as`, bound the inputs with `requires` or a guard, or qualify the type with `Wrapping`, `Saturating`, or `Trapping`." },
        ],
      },
    ],
  },
  {
    id: "ch-5",
    num: "VI",
    title: "The Boundary Wall",
    standfirst: "Machines never call OS syscalls or globals directly. Every external effect crosses a `boundary trait`, appears in `reaches`, and is wired to a provider by the build.",
    staves: [
      {
        title: "Reach is declared, then checked",
        figure: {
          kind: "code",
          code: `use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

machine start() reaches Console + ProcessExit {
    Console::write_line("through one wall");
    ProcessExit::exit_process(0);
}`,
          highlight: [5],
        },
        notes: [
          { for: 5, text: "`reaches Console + ProcessExit` is a conservative upper bound on the external services this machine (and anything it calls) may touch." },
          { for: 6, text: "`Console::write_line` targets the portable `Console` contract. A desktop build, a serial UART, and an in-memory test harness all satisfy the same call." },
          { for: 7, text: "Hosted `ProgramEntry` machines do not return an exit status value; they exit explicitly through `ProcessExit::exit_process`." },
          { text: "Audit `reaches` first during code review. A sorting helper or parser that asks to reach `FileSystem` or `ProcessExit` is a design bug." },
        ],
      },
      {
        title: "A contract, and who fills it",
        figure: {
          kind: "code",
          code: `public boundary trait Clock {
    machine now() -> u64
        reaches Clock;
}

public data FakeClock { }

machine FakeClock::now() -> u64
    satisfies Clock::now {
    1000u64
}

machine uptime() -> u64 reaches Clock {
    Clock::now()
}`,
          highlight: [1, 9, 14],
        },
        notes: [
          { for: 1, text: "`boundary trait` defines a portable external interface by signature only. `public` exports the declaration across package boundaries." },
          { for: 2, text: "Requirements can also publish operational envelopes (`suspends`, `blocks`, `crashes Trap`); callers must acknowledge them at the call site with `suspend` or `block`." },
          { for: 3, text: "A boundary requirement explicitly names `Clock` in its own `reaches` row. The trait name is never added implicitly." },
          { for: 6, text: "`FakeClock` is a stateless provider type. Providers hold no hidden state; they exist to anchor `satisfies` implementations." },
          { for: 9, text: "`satisfies Clock::now` binds `FakeClock::now` to that boundary requirement and checks that its signature and contract refine `Clock::now`." },
          { for: 14, text: "`uptime` calls `Clock::now()` against the trait and declares `reaches Clock`, staying decoupled from `FakeClock`." },
          { text: "In `source/library/std/console.omg`, `Console::read_byte` declares `blocks; crashes Trap;` — which is why callers write `block Console::read_byte()`." },
        ],
      },
    ],
  },
  {
    id: "ch-build",
    num: "VII",
    title: "The Build Machine",
    standfirst: "There is no TOML or YAML build manifest. `build.omg` is an ordinary checked Omega machine that declares the package kind, dependencies, and per-target entry bindings.",
    staves: [
      {
        title: "Wiring, declared",
        figure: {
          kind: "code",
          code: `machine build(builder: &mut Build) {
    Build::application(builder, "learn_omega");
    Build::depend(builder, Source::Path {
        location: "../../../Omega/source/library/std"
    });
    Build::bind(
        builder,
        windows_x86_64::ProgramEntry,
        start
    );
    Build::bind(
        builder,
        linux_x86_64::ProgramEntry,
        start
    );
    Build::bind(
        builder,
        macos_arm64::ProgramEntry,
        start
    );
}`,
          filename: "build.omg",
          companion: `use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

machine start() reaches Console + ProcessExit {
    Console::write_line("wired by the build machine");
    ProcessExit::exit_process(0);
}`,
          highlight: [6, 11, 16],
        },
        notes: [
          { for: 1, text: "`build.omg` defines `machine build(builder: &mut Build)`. It is parsed, type-checked, and evaluated under the same rules as application code." },
          { for: 2, text: "`Build::application` declares an executable root (libraries use `Build::package`; workspaces use `Build::member`). Mixing or repeating kinds is rejected." },
          { for: 3, text: "`Build::depend` pulls in the standard library from a source path or Git revision so `main.omg` can `use omega_language_std::console`." },
          { for: 6, text: "`Build::bind` connects a target's `ProgramEntry` slot to the free entry machine `start`. Naming `start` selects the entry without allocating hidden state." },
          { for: 8, text: "Because `ProgramEntry`'s schema comes from a dependency (`std`), the package manager requires an accepted `omega update` review recorded in `omega.lock`." },
          { for: 11, text: "Each hosted target (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) binds its own `ProgramEntry` to the same portable `start` machine." },
          { text: "Read `build.omg` first when reviewing a package: it shows every dependency, provider selection, and target entry point in one place." },
        ],
      },
    ],
  },
  {
    id: "ch-6",
    num: "VIII",
    title: "Text Is a Block",
    standfirst: "Omega has no format strings or heap-grown `String` in core. Fixed `[u8; N]` byte arrays and borrowed `&[u8]` slices make every buffer size and index proof explicit.",
    staves: [
      {
        title: "Pad it yourself",
        figure: {
          kind: "code",
          code: `use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

machine clear(line: &mut [u8; 28]) {
    transition { _ -> scan(line, 0u64) }

    state scan(line: &mut [u8; 28], k: u64) {
        transition k < 28u64 {
            true -> wipe(line, k)
            _    -> done()
        }
    }

    state wipe(line: &mut [u8; 28], k: u64) {
        line[k] = 32;
        transition { _ -> scan(line, k + 1u64) }
    }

    state done() { }
}

machine main() reaches Console + ProcessExit {
    let mut line: [u8; 28] =
        "this text is about to vanish";
    clear(&mut line);
    Console::write_line(&line[..]);
    ProcessExit::exit_process(0);
}`,
          highlight: [9, 16],
        },
        notes: [
          { for: 5, text: "`&mut [u8; 28]` borrows a mutable 28-byte array. Its static length `28` travels with the type." },
          { for: 8, text: "States do not capture enclosing locals, so `scan` takes both the borrowed `line` and the current index `k` explicitly." },
          { for: 9, text: "The guard `k < 28u64` is the bounds proof: on the `true` edge into `wipe`, the checker knows `k` is a valid index into `[u8; 28]`." },
          { for: 16, text: "`line[k] = 32` writes ASCII space (`0x20`). Zero-initialized bytes are NUL (`0x00`), so fixed-width output buffers are padded with spaces before printing." },
          { for: 17, text: "Because `k < 28u64` holds in `wipe`, `k + 1u64` is provably `<= 28` and cannot overflow `u64`." },
          { for: 24, text: "An ASCII string literal initializes a `[u8; N]` array of the exact matching byte length (here 28 bytes)." },
          { for: 27, text: "`&line[..]` borrows the full `[u8; 28]` array as a `&[u8]` byte slice for `Console::write_line`." },
          { text: "Formatting integers works the same way: divide by 10, take `% 10`, add `48` (`'0'`), and write into a space-padded `[u8; N]` buffer." },
        ],
      },
    ],
  },
  {
    id: "ch-7",
    num: "IX",
    title: "--check Is the Floor",
    standfirst: "`omega --check` proves type, domain, ownership, and reach contracts. It does not prove your algorithm computed the intended answer — run both the interpreter and native binary.",
    staves: [
      {
        title: "What the check does and doesn't say",
        figure: {
          kind: "table",
          head: ["Verification step", "What it actually establishes"],
          rows: [
            ["`omega --check` passes", "Types, range proofs, affine moves, exhaustive transitions, and `reaches` ceilings all hold statically."],
            ["`omega run --both` passes", "The Terminal Psi reference interpreter and the compiled native binary execute and agree on the outcome."],
            ["Output bytes inspected", "Stdout bytes, space padding, line endings, and the exit code match the specification."],
            ["`--check` alone", "Proves the program is safe to run, not that a constant return or wrong formula solved the problem."],
          ],
        },
        notes: [
          { text: "Choose test inputs whose expected result can only appear if the state machine genuinely ran its loop." },
          { text: "In the Omega repository, every capability fact row must pass `omega run --both` (Terminal Psi + native execution agreement), not `--check` alone." },
        ],
      },
      {
        title: "The exit code is an answer",
        figure: {
          kind: "code",
          code: `use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

// Expected exit: 70.
machine start() reaches Console + ProcessExit {
    let sorted: u64 in Wrapping = 64 + 6;
    Console::write_line("ten shelves, sorted");
    ProcessExit::exit_process(sorted as i32);
}`,
          highlight: [5],
        },
        notes: [
          { for: 5, text: "Executable samples declare an `// Expected exit: N` oracle comment at the top of `main.omg`, and the test harness verifies the exact status code." },
          { for: 7, text: "`u64 in Wrapping` opts into modular addition, and `as i32` converts the computed value for `exit_process`." },
          { for: 9, text: "Real samples encode their computed result in the exit status (`bubble_sort` exits with `10`, `collatz_sequence` with `111`, and `generic_counters` uses `3` on failure)." },
          { text: "When an agent writes a sample that always exits `0` without checking its computed result against the oracle, send it back." },
        ],
      },
    ],
  },
  {
    id: "ch-8",
    num: "X",
    title: "Working With the Agent",
    standfirst: "An agent can write Omega quickly once the state graph, data bounds, and capabilities are pinned. Your job is to specify those three up front and verify the run.",
    staves: [
      {
        title: "A brief that produces real Omega",
        figure: {
          kind: "prose",
          text: "Give the agent a four-part brief before it writes code: (1) the state names and transitions between them, (2) the `data` records with exact array lengths and range/overflow domains, (3) the allowed `reaches` services, and (4) the non-trivial expected exit code or output bytes. When the checker rejects a line, require the agent to reshape the state graph or tighten a contract rather than papering over the expression with `Wrapping`.",
        },
        notes: [
          { text: "Most agent failures come from inventing fields mid-state or slapping `in Wrapping` onto a loop counter whose termination depends on not wrapping." },
          { text: "Require both `omega --check` and an actual execution check (`omega run --both`) before accepting the task." },
        ],
      },
      {
        title: "The review checklist",
        figure: {
          kind: "table",
          head: ["Surface", "Question to ask"],
          rows: [
            ["`reaches` ceiling", "Does each machine declare only the external services it genuinely needs?"],
            ["`data` layout", "Are all persistent values stored in fixed-size records with explicit `where` or domain bounds?"],
            ["Borrowing (`&` / `&mut`)", "Are parameters borrowed `&mut` only when the machine actually mutates them?"],
            ["State frontier", "Do state names reflect domain steps, and does every loop edge shrink a clear measure?"],
            ["Transition coverage", "Are case dispatches exhaustive without silent catch-all arms hiding unhandled variants?"],
            ["Proof obligations", "Are bounds the true domain range rather than arbitrary buffer capacities or lazy `Wrapping`?"],
            ["Execution oracle", "Did the compiled binary run, and do the stdout bytes and exit code match the oracle?"],
          ],
        },
        notes: [
          { text: "Seven checks. If the code clears all seven and passes `omega run --both`, the machine is solid." },
        ],
      },
    ],
  },
  {
    id: "ch-capstone",
    num: "XI",
    title: "One Whole Program",
    standfirst: "Everything in one twenty-line program: it counts the base-10 digits of `2026` across a two-state loop and exits with status `4`.",
    staves: [
      {
        title: "The program, whole",
        figure: {
          kind: "code",
          code: `use omega::language::core::process_exit;
use omega_language_std::process_exit;
use omega_language_std::console;

// Expected exit: 4.
machine start() reaches Console + ProcessExit {
    transition { _ -> count(2026u64, 0u8) }

    state count(n: u64, digits: u8 in Wrapping) {
        transition n >= 10u64 {
            true  -> count(n / 10u64, digits + 1u8)
            _     -> report(digits + 1u8)
        }
    }

    state report(digits: u8 in Wrapping) {
        Console::write_line("counted the digits");
        ProcessExit::exit_process(digits as i32);
    }
}`,
          highlight: [9, 11],
        },
        notes: [
          { for: 6, text: "Read the capability ceiling first: `start` may write to `Console` and terminate via `ProcessExit`, and nothing else." },
          { for: 7, text: "Setup finishes with an unconditional `transition` into `count(2026u64, 0u8)`, seeding the loop's initial state parameters." },
          { for: 9, text: "`digits: u8 in Wrapping` qualifies the accumulator with wrapping arithmetic, and every `u8` widens losslessly into `i32` at exit." },
          { for: 10, text: "`n >= 10u64` partitions the loop: values with at least two digits left continue; single-digit remainders finish in `report`." },
          { for: 11, text: "The back-edge `count(n / 10u64, digits + 1u8)` divides `n` by `10u64` (a nonzero constant divisor), strictly shrinking `n` on every iteration." },
          { for: 18, text: "Exits with `4` — the four digits of `2026` — matching the `// Expected exit: 4` oracle at the top of the file." },
          { text: "Because `n` and `digits` travel as state parameters across the loop, the program needs no `data` block at all." },
        ],
      },
      {
        title: "What the check did not tell you",
        figure: {
          kind: "prose",
          text: "`omega --check` proved that `10u64` is a nonzero divisor, `digits + 1u8` has a defined overflow policy, `digits as i32` is in range, every transition arm lands in a valid state, and only `Console` and `ProcessExit` are reached. It did not prove that `2026` has 4 digits rather than 5 — running the binary and checking exit code `4` against the oracle closes that loop.",
        },
        notes: [
          { text: "Static proof guarantees the machine's invariants and safety boundaries; execution against an oracle verifies its domain answer." },
        ],
      },
    ],
  },
];
