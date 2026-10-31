# Joint lexicographic ranks

Owner: [NESTED-MACHINE-UNIFICATION](../../../../TASKS.md).

Fact: joint private arrivals retain a lexicographic rank while lower components
reset after a strict higher-component decrease. Every place uses the same bound
and operation: `Progress { outer: 1, inner: 2, canary: 641 }` visits four inner
steps and advances the caller counter3→7. Already-complete0,0 leaves it unchanged.
A `[copy]` declaration admits the constant and copied-field transports; it does
not supply progress or a tuple ordering proof.

- [local](local/main.omg), [zero-step twin](local/invalid/main.omg).
- [field](field/main.omg), [zero-step twin](field/invalid/main.omg).
- [parameter](parameter/main.omg), [zero-step twin](parameter/invalid/main.omg).
- [call_result](call_result/main.omg), [zero-step twin](call_result/invalid/main.omg).
- [match_join](match_join/main.omg), [zero-step twin](match_join/invalid/main.omg).
- [const_name](const_name/main.omg), [zero-step twin](const_name/invalid/main.omg).
- [other_state](other_state/main.omg), [zero-step twin](other_state/invalid/main.omg).
- [other_machine](other_machine/main.omg), [zero-step twin](other_machine/invalid/main.omg).
- [target_body](target_body/main.omg), [zero-step twin](target_body/invalid/main.omg).

The [frozen original probe](parameter/probe/main.omg) is byte-identical to the
first failing affine-carrier program. Exact optimized fixed base
`a91a8de86514f5b0ba71509b3af4d656916d6062` refuses it and all nine well-formed
places because a write, call or alias invalidates the entry-relative ranking.
The source repair accepts all nine places through captured tuple comparisons.
The first attempt still fails native lowering at `state graph: natural ranks`;
this change carries the complete tuple through that boundary. All nine places
now execute with native/Terminal agreement at exit0 on macOS ARM64; all nine
zero-step twins reject. The focused sample run has16agreements and12intended
refusals, including the controls and three existing scalar neighbors. Pinned
corpus acceptance has13native/Terminal agreements and30source outcomes
(13checked,17expectedrefusals, including five existing nonincrease controls).
Three tuple wire tests pass. A throwaway100,000-step instance also agrees in
both engines and its native image exits0 under a256KiB stack limit; it is a
physical stack check, not a separate fact row.

Every zero-step twin changes only the inner decrement from1to0. A strict edge
elsewhere cannot excuse that preserving cycle. Pins are
`nested_machine_lexicographic_rank_<place>_{exit,rejected}` under
`tests/omega/{pass,fail}/facts/`.

The focused controls challenge the operation rather than its spelling:

- [Self-call alias](controls/self_alias/main.omg) carries a computed local record
  into an ordinary self-arrival after a conditional counter update.
- [Scalar return](controls/scalar_return/main.omg) keeps a nonempty authored scalar
  telescope and returns the advanced counter; private rank phis must follow the
  authored slots and preserve the scalar completion.
- [Negative prefix](controls/negative_prefix/main.omg) holds an `i8` component at
  -1 while its `u64` suffix descends. Its [preserving twin](controls/negative_preserving/main.omg)
  must reject the weak cycle, not acquire progress from a false zero-floor premise.
- [Reset without prefix descent](controls/reset_without_outer_decrease/main.omg)
  rejects the increasing suffix when the earlier component remains unchanged.
- [Unsafe suffix](controls/unsafe_suffix/main.omg) rejects its own underflow even
  though an earlier component decreases. A lexicographic short circuit never
  excuses unformed operands or carriers.

Establishment and soundness: selected resolved field identities determine the
ordered components. Source requires and guards form every partial operation.
All components are independently typed and formed before a strict prefix may
ignore the suffix's order. Natural views retain their zero floor; a fixed-word
view proves its finite carrier floor. Proposed floors are goals, not premises.
See [termination](../../../../wiki/spec/language/termination.md#ranking) and
[control flow](../../../../wiki/spec/terminal-psi/control_flow.md).

Identity and lifetime: entry observations freeze the whole ordered rank before
prefix effects; exact immutable atoms and already-proved bounds travel together.
The ordinary completed-prefix ledger supplies ordered actuals and projections.
Counter writes, local aliases and ownership transfers cannot reinterpret a saved
source value as later storage. Each destination forms in its own completed scope.

Composition: every edge proves lexicographic nonincrease and records strictness;
the preserving-edge graph must be acyclic. The complete component owns that
judgment, including self-calls; struct-literal and subtract-spelling recognizers
supply no alternative proof. A local tuple query records a pending component
obligation, never a termination proof; only complete component admission can
discharge it. Internal state cycles cannot acquire a proof from this deferral.
Same-shaped participants share this exact view;
the open syntax question for differently shaped mutual participants is unrelated.

Terminal production: admission and executable plans each own ordered component
spans. Scalar root roles retain component ordinals, each cyclic header has the
complete private phi tuple, and the initial invocation observes all components
before entering the header. Completed arrivals and continuation namespaces carry
every component. The wire format retains each integer type and SSA value and
rejects missing or differently typed suffixes. Psi rewrites preserve all named
values; the optional unsigned-countdown optimization declines nonsingletons.

Omega realization: tuple observations use ordinary scalar computations and block
arguments, layout, ABI, register homes, frame realization and image emission.
The counter, both active/idle paths and canary641 expose missing updates,
incorrect transfers or mismatched slots. The scalar-return and mixed signed
controls challenge the shared parameter/result path. No packed scalar rank or
separate emitter is introduced. Other-host execution and independent proof-kernel
qualification remain unverified; broad parent acceptance and product migration
remain open. Raw producer hashes, source patches and observations live in
`build/joint-lexicographic/`.
