// Chapter content for Learn Omega. Every code block is real Omega, checked
// against the compiler built from current main (`omega --check` on each
// fixture under scratch/verify/; see tools/verify.ts). No `self` anywhere: Omega has no implicit receiver;
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
          { for: 3, text: "Brings in the `Console` boundary trait. The standard library supplies the provider for each hosted target, and `build.omg` can select another one." },
          { for: 5, text: "`machine` declares a state machine with no implicit state or `self`. `reaches Console + ProcessExit` is its service ceiling: without that clause, it cannot touch the outside world." },
          { for: 6, text: "`Console::write_line` calls the boundary contract statically. The source never names a platform handle or syscall." },
          { for: 7, text: "`ProcessExit::exit_process(0)` terminates the process domain with status 0. Printing never implies permission to exit, so the signature names both services." },
        ],
      },
      {
        title: "What the supervisor reads",
        figure: {
          kind: "prose",
          text: "Review agent-written Omega in three passes: first the `reaches` clause (what outside services it can touch), then the `data` declarations (what memory it owns and its bounds), and finally the graph of machines (how control moves). If those three are honest, the implementation inside each machine is boxed in by the checker.",
        },
        notes: [
          { text: "Capabilities first, data layout second, transitions between machines third. Every review in this guide follows that order." },
        ],
      },
    ],
  },
  {
    id: "ch-2",
    num: "II",
    title: "States, Not Ifs",
    standfirst: "Branching is a `transition` that picks which machine runs next, or a `match` that picks a value. Looping is a machine that calls itself as its last action, so the stack does not grow.",
    staves: [
      {
        title: "A guard picks the next machine",
        figure: {
          kind: "code",
          code: `machine choose(flag: bool) -> u64 {
    transition flag {
        true  -> yes()
        _     -> no()
    }

    machine yes() -> u64 { 70u64 }
    machine no()  -> u64 { 71u64 }
}

machine main() -> u64 { choose(true) }`,
          highlight: [2, 3, 4],
        },
        notes: [
          { for: 2, text: "`transition` evaluates its subject once and runs only the first arm that matches. The chosen arm is the last thing the machine does, so its result becomes the machine's result." },
          { for: 4, text: "`_` covers every remaining value. Transition arms must cover every possible value, so every path has a destination." },
          { for: 7, text: "`yes` is a machine nested inside `choose`. Nesting only limits where the name is visible; `yes` has its own signature and contract and is not a closure over `choose`'s parameters. The old `state` declaration is retired: there is one kind of machine." },
          { for: 11, text: "A machine that holds no persistent state needs no `data` declaration. Its inputs and return type are its entire interface." },
        ],
      },
      {
        title: "A machine that calls itself is a loop",
        figure: {
          kind: "code",
          code: `machine countdown(n: u64) -> u64 {
    transition n > 0u64 {
        true  -> tick(n)
        _     -> n
    }
    machine tick(n: u64) -> u64
    requires n > 0u64
    {
        let next: u64 = n - 1;
        transition next > 0u64 {
            true  -> tick(next)
            _     -> next
        }
    }
}

machine main() -> u64 { countdown(5u64) }`,
          highlight: [12],
        },
        notes: [
          { for: 8, text: "A nested machine is checked against its own contract, not against whatever was true where it was called. `tick` writes `requires n > 0u64` itself; the guard on line 2 is what lets the call on line 3 establish it." },
          { for: 10, text: "Unsigned `n - 1` is legal because `tick`'s own `requires` says `n > 0`. Remove the `requires` and exact-arithmetic checking rejects the subtraction." },
          { for: 11, text: "`tick` tests `next > 0` explicitly: the call `tick(next)` on the next line must establish `tick`'s `requires` for its new argument." },
          { for: 12, text: "`tick(next)` calls `tick` again with a smaller argument, as the last thing the arm does, so it compiles to a jump and the stack does not grow. Self-calls in last position replace `while` and `for` loops." },
          { text: "When reviewing a loop, check two things: the contract that proves the step cannot underflow, and the value that strictly decreases on every back-edge. A machine that promises to finish also writes `terminates by n -> Nat::Descending` (see the termination spec)." },
        ],
      },
      {
        title: "Reading the graph",
        figure: {
          kind: "prose",
          text: "Ask the agent for the list of nested machines and the transitions between them before asking for code. Good machine names describe stages of the domain workflow (`scan`, `wipe`, `done`); vague names (`step2`, `helper`, `process`) mean the control flow has not been designed yet.",
        },
        notes: [
          { text: "Machine names are the cheapest design review you can run, and the fastest way to catch a muddled design." },
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
          { for: 6, text: "`[Room; 2]` nests two `Room` records inline inside `Level`. The element count is part of the type, so `Level` has a fixed size." },
          { for: 7, text: "`i32 in Wrapping` qualifies the field with modular two's-complement arithmetic, allowing `level.ticks + 1` without an upper-bound proof." },
          { for: 10, text: "`&mut Level` lends mutable access to `level` for the duration of the call. There is no implicit `self` parameter, and the data is always an ordinary parameter." },
          { for: 11, text: "Constant indices `1` and `0` are statically in bounds against `[Room; 2]` and `[u8; 4]`, so the checker discharges both array bounds automatically." },
          { for: 16, text: "Record construction names every field. `let mut` is required here: writing a local's fields or lending it with `&mut` or `&write` both need a `mut` binding, and without it the call on line 23 is rejected (spec: owned local mutability)." },
          { text: "If an agent starts threading five loose parameters through every nested machine, stop and have it group that state into a `data` record with explicit field types." },
        ],
      },
    ],
  },
  {
    id: "ch-cases",
    num: "IV",
    title: "Choice Is a Value Too",
    standfirst: "Sum types are `data` declarations with `case` members. You inspect them through exhaustive `transition` or `match` dispatch, and the data's `requires` clause can constrain each case's payload.",
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
          { for: 21, text: "A record literal supplies every field. There is no struct-update shorthand (`..old`): a final bare `..` only waives fields, it never copies them from another value, so `advance` names every field." },
          { for: 23, text: "Because `advances` carries `i32 in Wrapping`, the addition uses modular wrapping semantics automatically." },
          { for: 34, text: "`light` was already passed on line 32, and `[copy]` lets line 34 use it again. Without `[copy]`, the checker rejects the second use because a moved affine binding holds no value." },
          { text: "`match` and `transition` share one pattern vocabulary, including case patterns and payload bindings: `match` produces a value, while `transition` chooses what runs next." },
        ],
      },
      {
        title: "Cases carry cargo",
        figure: {
          kind: "code",
          code: `data Reply
    requires (self in Reply::Miss)
       || ((self in Reply::Hit) && self.value >= 0 && self.value <= 255)
{
    case Hit(value: i32);
    case Miss;
}

machine read(reply: Reply) -> i32 {
    transition reply {
        Reply::Hit { value } -> keep(value)
        Reply::Miss          -> miss()
    }

    machine keep(value: i32) -> i32
    requires value >= 0 && value <= 255
    {
        value
    }
    machine miss() -> i32 { 0 }
}

machine main() -> i32 {
    let reply: Reply = Reply::Hit { value: 66 };
    read(reply)
}`,
          highlight: [2, 3, 10],
        },
        notes: [
          { for: 2, text: "A `requires` clause on the data states what holds for every value; `self` is the whole value. `Reply::Miss` carries no payload, so nothing more is asked of it." },
          { for: 3, text: "A case test on the left of `&&` permits reading that case's payload on the right. Constructing `Reply::Hit` must prove `value` is in `0..=255`; matching `Hit` hands that fact back." },
          { for: 10, text: "Subject-form `transition reply` dispatches on the active case of `reply`. Every declared case must be handled." },
          { for: 11, text: "`Reply::Hit { value }` destructures the payload by field name and calls `keep(value)`. The `0..=255` fact is known here, but `keep` is checked against its own contract, so it states that range as its own `requires`." },
          { for: 12, text: "`Reply::Miss` has no payload fields, so the variant path alone matches it." },
          { for: 16, text: "`keep`'s `requires` is what its body may rely on. The call on line 11 must establish it, and the case invariant is what lets it." },
          { for: 24, text: "The literal `66` satisfies `0..=255` at compile time. Change it to `300` and construction is rejected: construction is the gate for the data's `requires`." },
          { text: "Standard-library `ByteRead` uses this exact pattern: `case Eof;` and `case Byte(value: i32);` under a data `requires` that bounds the byte to `0..=255`." },
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

    machine finish(saved: u16) -> u16 {
        saved
    }
}`,
          highlight: [2, 3, 7],
        },
        notes: [
          { for: 2, text: "`requires` states a precondition the caller must establish before invoking `bounded`; the body may rely on it. It is a proof obligation at each call, not a runtime check." },
          { for: 3, text: "`ensures` states the postcondition guaranteed on normal return, using the reserved name `result` for the returned value." },
          { for: 5, text: "`finish(input)` is the last thing `bounded` does, so `finish`'s result becomes `bounded`'s result." },
          { for: 7, text: "How the checker proves `result <= input`: (1) `finish` is in the same file, so the checker reads its body, which returns `saved` unchanged; (2) the call passes `input` as `saved`, so the result equals `input`; (3) `input <= input`, which is what `bounded` promised. Nothing needed an `ensures` on `finish` here. Code in another package sees only a machine's written contract, so a machine meant for outside callers writes the guarantees they need. This matters because a promise is only as good as the proof behind it." },
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
          code: `error: exact arithmetic in machine \`rename\` state \`entry\`
  assignment may overflow \`i32\`: the operands are not provably in range
  (decision 17 -- exact arithmetic is a proof obligation).
  Widen with an \`as\` cast to a larger type, constrain the operands'
  range (bound a parameter with a \`requires\` clause, or narrow a value
  with a dominating guard), or opt into a defined-overflow domain
  (\`i32 in Wrapping\`/\`Saturating\`/\`Trapping\`).`,
          highlight: [2, 3],
        },
        notes: [
          { for: 1, text: "The `omega --check` diagnostic when `in Wrapping` is removed from `ticks` in Chapter III. The compiler prints it on one line; line breaks are added here, and it ends with the source position." },
          { for: 2, text: "`may overflow` means the range proof is missing, not that a specific input overflowed. Every Exact operation owes a proof that its result fits the carrier." },
          { for: 3, text: "`decision 17` is the compiler's own label for the rule. The rule itself is in `wiki/spec/language/numeric_values.md`: there is no implicit widening, and Exact arithmetic must fit its carrier." },
          { for: 4, text: "Every refusal lists the legal repairs: widen with `as`, bound the inputs with `requires` or a guard, or qualify the type with `Wrapping`, `Saturating`, or `Trapping`." },
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
          { for: 7, text: "This entry machine returns no value; it ends the process explicitly through `ProcessExit::exit_process`. The compiler rejects an entry machine that returns a value." },
          { text: "Audit `reaches` first during code review. A sorting helper or parser that asks to reach `Console` or `ProcessExit` is a design bug." },
        ],
      },
      {
        title: "A contract, and who fills it",
        figure: {
          kind: "code",
          code: `public boundary trait Clock {
    machine now() -> u64
        reaches Clock
        crashes none;
}

public FakeClock satisfies Clock {
    machine now() -> u64 {
        1000u64
    }
}

machine uptime() -> u64 reaches Clock {
    Clock::now()
}`,
          highlight: [1, 7, 13],
        },
        notes: [
          { for: 1, text: "`boundary trait` defines a portable external interface by signature only. `public` exports the declaration across package boundaries." },
          { for: 2, text: "Requirements can also publish operational envelopes (`suspends`, `blocks`, `crashes Trap`); callers must acknowledge them at the call site with `suspend` or `block`." },
          { for: 3, text: "A boundary requirement explicitly names `Clock` in its own `reaches` row. The trait name is never added implicitly." },
          { for: 7, text: "`FakeClock satisfies Clock` declares a named implementation. It is not a data type with machines attached to it, and it holds no state: anything that must outlive a call is a value the service returns and the caller passes back." },
          { for: 8, text: "The implementation's machine inherits `Clock::now`'s contract, and the checker verifies the body refines it." },
          { for: 13, text: "`uptime` calls `Clock::now()` against the trait and declares `reaches Clock`, staying decoupled from `FakeClock`. The build selects which implementation answers `Clock`." },
          { text: "In `source/library/std/console.omg`, `Console::read_byte` declares `blocks` and `crashes Trap`, which is why callers write `block Console::read_byte()`." },
        ],
      },
    ],
  },
  {
    id: "ch-build",
    num: "VII",
    title: "The Build Machine",
    standfirst: "Build configuration is Omega source, not a separate manifest format. `build.omg` is an ordinary checked machine that declares the package kind, dependencies, and per-target entry bindings.",
    staves: [
      {
        title: "Wiring, declared",
        figure: {
          kind: "code",
          code: `machine build(builder: &mut Build) {
    Build::application(builder, "learn_omega");
    let std_registration: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path {
        location: "../../source/library/std"
    }));
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
        linux_arm64::ProgramEntry,
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
          { for: 3, text: "`Build::dependency` registers the standard library from a source so `main.omg` can `use omega_language_std::console`. Dependencies come only from this registration; imports never add one." },
          { for: 6, text: "`Build::bind` connects a target's `ProgramEntry` slot to the free entry machine `start`. Naming `start` selects the entry without allocating hidden state." },
          { for: 8, text: "A project with dependencies records its accepted resolution in `omega.lock`; `omega packages update` acquires and reviews it (spec: package acceptance)." },
          { for: 11, text: "Each hosted target (`windows_x86_64`, `linux_x86_64`, `linux_arm64`, `macos_arm64`) binds its own `ProgramEntry` to the same portable `start` machine. A required slot with no binding for the selected target is an error." },
          { text: "Read `build.omg` first when reviewing a package: it shows every dependency, provider selection, and target entry point in one place." },
        ],
      },
    ],
  },
  {
    id: "ch-6",
    num: "VIII",
    title: "Text Is a Block",
    standfirst: "Core has no format strings or `String` type; text is bytes. Fixed `[u8; N]` byte arrays and borrowed `&[u8]` slices make every buffer size and index proof explicit.",
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

    machine scan(line: &mut [u8; 28], k: u64) {
        transition k < 28u64 {
            true -> wipe(line, k)
            _    -> done()
        }
    }

    machine wipe(line: &mut [u8; 28], k: u64) {
        line[k] = 32;
        transition { _ -> scan(line, k + 1u64) }
    }

    machine done() { }
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
          { for: 8, text: "Nested machines do not capture the enclosing machine's parameters or locals, so `scan` takes both the borrowed `line` and the current index `k` explicitly." },
          { for: 9, text: "The guard `k < 28u64` is the bounds proof. `wipe` writes no `requires`, so the compiler infers one from its body (`k < 28`) and checks it at every call; on the `true` edge into `wipe` the call establishes it. Once you write a `requires` yourself, it must cover everything the body needs." },
          { for: 16, text: "`line[k] = 32` writes ASCII space (`0x20`). Zero-initialized bytes are NUL (`0x00`), so fixed-width output buffers are padded with spaces before printing." },
          { for: 17, text: "Because `wipe` is only entered with `k < 28u64`, `k + 1u64` is provably `<= 28` and cannot overflow `u64`." },
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
            ["`omega run --both` passes", "The Psi reference interpreter and the compiled native binary execute and agree on the outcome."],
            ["Output bytes inspected", "Stdout bytes, space padding, line endings, and the exit code match the specification."],
            ["`--check` alone", "Proves the program is safe to run, not that a constant return or wrong formula solved the problem."],
          ],
        },
        notes: [
          { text: "Choose test inputs whose expected result can only appear if the machine genuinely ran its loop." },
          { text: "In the Omega repository, every capability fact row must pass `omega run --both` (Psi + native execution agreement), not `--check` alone." },
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
          { for: 7, text: "`u64 in Wrapping` opts into modular addition, and `as i32` converts the computed value to the `i32` status `exit_process` takes." },
          { for: 9, text: "Real samples encode their computed result in the exit status (`bubble_sort` exits with `10`, `collatz_sequence` with `111`, and `generic_counters` with `16`)." },
          { text: "When an agent writes a sample that always exits `0` without checking its computed result against the oracle, send it back." },
        ],
      },
    ],
  },
  {
    id: "ch-8",
    num: "X",
    title: "Working With the Agent",
    standfirst: "An agent can write Omega quickly once the machine graph, data bounds, and capabilities are pinned. Your job is to specify those three up front and verify the run.",
    staves: [
      {
        title: "A brief that produces real Omega",
        figure: {
          kind: "prose",
          text: "Give the agent a four-part brief before it writes code: (1) the machine names and the transitions between them, (2) the `data` records with exact array lengths and range/overflow domains, (3) the allowed `reaches` services, and (4) the non-trivial expected exit code or output bytes. When the checker rejects a line, require the agent to reshape the machine graph or tighten a contract rather than papering over the expression with `Wrapping`.",
        },
        notes: [
          { text: "Most agent failures come from inventing fields mid-machine or slapping `in Wrapping` onto a loop counter whose termination depends on not wrapping." },
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
            ["Machine graph", "Do machine names reflect domain steps, and does every loop edge shrink a clear measure?"],
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
    standfirst: "Everything in one twenty-line program: it counts the base-10 digits of `2026` with a nested machine that calls itself, and exits with status `4`.",
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

    machine count(n: u64, digits: u8 in Wrapping) {
        transition n >= 10u64 {
            true  -> count(n / 10u64, digits + 1u8)
            _     -> report(digits + 1u8)
        }
    }

    machine report(digits: u8 in Wrapping) {
        Console::write_line("counted the digits");
        ProcessExit::exit_process(digits as i32);
    }
}`,
          highlight: [9, 11],
        },
        notes: [
          { for: 6, text: "Read the capability ceiling first: `start` may write to `Console` and terminate via `ProcessExit`, and nothing else." },
          { for: 7, text: "Setup finishes with an unconditional `transition` into `count(2026u64, 0u8)`, seeding the loop's initial arguments." },
          { for: 9, text: "`digits: u8 in Wrapping` qualifies the accumulator with wrapping arithmetic, and `digits as i32` is an explicit conversion: Omega has no implicit widening, and every `u8` value fits an `i32`." },
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
