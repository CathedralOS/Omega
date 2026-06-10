# Appendix: Open Questions

This page tracks design pressure that is not fully nailed down yet.

## Current Answers

- `const` parameters are compile-time values, proof constants, or both. Omega should use every fact it can soundly know.
- `&mut self` is the working spelling for attached machines and states that need machine state. The goal is not to be different for the sake of being different.
- `machine` is the callable boundary. Calling a machine creates the callable activation; transitioning to an internal `state` does not.
- States are graph labels inside the current machine. They may take arguments and participate in return-value compatibility, but they are reached by `transition`, not by normal call syntax.
- Terminal value completion is useful: a final value completes the active machine with a value, while `transition { _ -> state_name(args) }` jumps to an internal state.
- Relax obligations are compile-time proof obligations. The runtime should not carry hidden invariant state unless a debug/proof artifact explicitly asks for it.
- Target signatures define the invariants they accept. Either the caller can prove the handoff satisfies the signature, or the transition is illegal.
- `domain` names a type-scoped proof predicate over an existing value. Domains
  may have explicit `when` classifiers for cheap domain-pattern matching, but
  Omega should not inject hidden runtime tags to make ambiguous domains
  distinguishable. Matching a data value with `Type::Domain` checks that
  domain and adds its facts to the selected arm.
- `match` is for value selection. `transition` is for control movement. Conditional transitions name a scrutinee; anonymous `transition { _ -> target }` is only for unconditional jumps.
- Borrowed slices should use Rust-like `&[T]` / `&mut [T]` surface syntax.
  They are built-in borrowed views with proof-visible facts such as `len` and
  type-scoped invariant parameters such as `&[T, [non_empty]]`; indexing is
  valid when the current facts prove the index is inside the slice bounds.
- Old bracketed refinement syntax is dead. Range-heavy proof vocabulary lives directly in contracts, using Rust-style ranges like `1..=100` and `min..=max`. The inclusive/exclusive forms are now resolved: `a..b` is exclusive and `a..=b` is inclusive, with `a..=b` normalizing to `a..(b+1)`. Against a length `len`, an exclusive end requires `b <= len` and an inclusive end requires `b < len` (so inclusive-end validity equals index validity); a non-empty inclusive range establishes a `non_empty` fact. These are the same `..` / `..=` forms used for subslicing.
- Text type naming is resolved. The owned type is `String`; the borrowed window
  is `&string` (lowercase). Casing distinguishes the owner from the window. The
  earlier `Str` / `StrView` (and `&str`) naming is retired.
- Omega should distinguish proof numbers from machine numbers. `UInt`, `Int`, and `Real` are useful as mathematical/spec types, while `i32`, `u64`, `f32`, and similar types are concrete machine representations with explicit proof obligations.
- Machine integer arithmetic should probably default to exact/proven semantics. Weaker behavior such as `wrapping`, `trap`, `saturating`, or `checked` should be explicit because each mode changes proof obligations and runtime behavior.
- Omega's proof vocabulary should distinguish facts, requirements, guarantees, obligations, invariants, contracts, and boundary. Values carry facts; operations have contracts; contracts create obligations; boundary names the authority for accepting unproved guarantees.
- Termination should be an explicit proof claim such as `terminates`, with
  nested progress clauses such as `decreases value -> OrderOrMeasure`. A
  terminating root like `Main::main` should implicitly require every reachable
  recursive/cyclic path to prove progress through a well-founded ranking view
  such as naturals, slice length, or a named domain/type-specific order.
- Termination syntax sugar and custom ranking views are resolved. The core shape
  stays `terminates { decreases value -> OrderOrMeasure; }`; plain
  `decreases value` uses the default descending-naturals order. Built-in views
  such as `Slice::Length` and descending naturals remain automatic. A custom
  well-founded ordering is declared with a dedicated `measure` keyword as a
  standalone item, not by abusing an `operator` declaration: a `measure` is a
  function from the decreasing value into a well-founded domain such as `usize`,
  e.g. `measure Card::PowerOrder(card: Card) -> usize { card.power }`.
  `lexicographic { a, b, ... }` declares an ordered tuple compared left-to-right
  (e.g. `measure Quest::Difficulty lexicographic { tier, remaining_steps }`), and
  multiple named measures per type are allowed.
- Inline assembly should be parsed as target assembly under Omega's stricter accepted subset rather than bypassing the language. Assembly jumps are only valid if they satisfy Omega's state-transition rules, and assembly memory/register effects must be declared or inferred from known instruction contracts.
- Semantic states remain branch-free. Source-level mid-state transitions may exist for early exits, but the compiler lowers them into generated branch-free sub-states or basic blocks with explicit edges and cleanup.
- A machine enters at the top of its body. Machines may still need target-specific startup rules, but function callability and runtime startup should not be conflated.
- Omega should avoid reserving keywords aggressively. Prefer contextual keywords when grammar position is enough, especially for words like `entry`, `where`, `boundary`, `requires`, and `ensures`. Fully reserved words should be rare and justified by parser clarity, safety, or proof semantics.
- Zero is initialization. The all-zero bit pattern is a valid, memory-safe
  inhabitant of every `data` and `enum` type; invariants and domains describe
  ESTABLISHED values, so a zeroed object carries no facts but is never
  undefined behavior. No niche-style layout optimization may repurpose the
  zero pattern. Enum tag `0` is the first declared variant (declare the
  empty case first). See Memory Layout And ABI.
- Enums are Rust-like sum types. Payload-carrying variants
  (`enum Command { None, Move(Direction) }`) are the committed direction;
  payload-less enums work today. Tag-prefixed layout, exhaustive matching,
  zero variant first.
- Atomics are Rust/C11-like: dedicated core atomic types with explicit
  orderings (`Relaxed`, `Acquire`, `Release`, `AcqRel`, `SeqCst`) on every
  operation; the sanctioned shared-mutation carve-out from exclusive `&mut`.
  Working assumption: compiler intrinsics, not boundary operators. See
  Concurrency.
- Volatile/device-memory access goes through boundary operators with volatile
  contracts (each access exactly once, declared width, program order among
  volatile accesses on a region), not a type qualifier. Hardware ordering is
  the boundary contract's job, not volatility's. See Memory Layout And ABI.
- Freestanding (ring-0) targets have an EMPTY host-provider set; the lowest
  boundary declares facts about hardware (MMU, MSRs, MMIO regions, interrupt
  control), and the asm instruction-contract subset is the implementation
  vehicle for many such providers. See Capabilities, Effects, And Boundaries.

## Still Open

- Can the compiler infer result bounds from `match` and `transition` partitions without explicit annotations?
- How much domain classifier/checker inference should Omega attempt beyond
  explicit `when` clauses and executable domain bodies?
- What exact source form should core operator declarations use for `[]`,
  subslicing, arithmetic, and string concatenation?
- Which core semantic types should be browsable source declarations, and which
  primitive carriers should remain compiler-managed? Current direction:
  `Array`, `Vec`, `Slice`, `String`, and `&string` are public core concepts;
  `Ptr` and descriptor-like carriers sit at the low-level boundary.
- How should Omega express and prove sequence-wide domains over runtime text,
  such as `String::Utf8` or `String::NoNul`, without turning ordinary string
  handling into a byte-level proof tax?
- When domains can participate in operator resolution, what exact ambiguity
  rules should apply, and which concepts should remain ordinary value domains
  versus a separate evaluation-mode/policy system?
- Should relax ever permit graph-edge proof debt, or should it remain strictly
  lexical and non-transitioning?
- How explicit should weakened machine invariants be in target state signatures?
- Can typed state clusters suspend across ticks, or must they complete in one scheduling turn?
- What syntax should Omega use for float optimization permissions, separate from float invariants?
- Which float properties should be first-class invariants: `finite`, `non_nan`, `normal`, signed-zero policy, or something else?
- What exact spelling should arithmetic modes use: scoped policy, operator variants, or domain-sensitive operator resolution for semantic quantity domains?
- Does `checked` arithmetic return `Option`, `Result`, a language-specific checked value, or require explicit operator forms?
- How should `Real` contracts lower when called with `f32` or `f64`: explicit `approx<Real, eps=...>`, compiler-inferred error bounds, or named approximation policies?
- Should inline assembly allow local labels and internal jumps, or only structured exits that map to Omega transitions?
- What is the minimum contract syntax for assembly clobbers, memory effects, target features, and emitted invariants?
- Which assembly instructions belong in the first accepted subset for each target, and what exact contracts should each instruction emit?
- When should manual assembly contracts be allowed to supplement known instruction contracts, and when should they be rejected as too opaque?
- Which words must be globally reserved, and which should remain contextual keywords only?
- How should imported library/syscall signatures and target bindings describe native operand lowering, so `Stdout.write` and `Process.exit` are not compiler-special string matches?
- Should the language eventually support explicit tail calls into machines, and if so what spelling avoids confusing them with ordinary `-> state()` transitions?
- Enum payloads: pattern-binding syntax in `transition` vs `match` arms,
  generic payloads, exact tag-prefixed layout rule.
- Zero-is-initialization follow-ups: the zero-excluding-invariant lint, and
  whether any type may opt into constructed-only semantics.
- Atomics follow-ups: standalone fences, `compare_exchange` failure-ordering
  surface, and how the concurrency proof model treats relaxed visibility.
- Volatile/hardware follow-ups: the operator surface for MMIO regions, `repr`
  spelling for packed/explicit-offset hardware structures, untagged unions.
- Freestanding follow-ups: target-declaration shape for "no host", the entry
  provider contract (firmware handoff state), interrupt-handler entry into a
  machine graph, section/physical-address placement in image emission.
- What is the component artifact + cross-component ABI for separately
  compiled, hot-swappable machines? (Inventory of current whole-program
  assumptions: wiki/architecture/whole_program_assumptions.md.)
