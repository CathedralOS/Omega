# Tasks

Compiler/runtime work surfaced by the canary ladder, the dungeon sample, and
the current language-design push.

The major direction is now clearer: Omega needs a small set of browsable core
semantic concepts, plus a deliberately private compiler/runtime layer where
the unsafe representation work happens. Users should be able to navigate to
`Slice::Length` or an indexing operator contract and understand the language
meaning, without needing access to pointer descriptor internals.

## Architecture Tracks

- [ ] Core semantic surface
  Create source-visible declarations for the core concepts that are currently
  mostly compiler knowledge.
  Landed:
  - initial browsable modules for `Slice`, `Vec`, `Array`, and text direction
    under `omega/language/core`
  - initial browsable `Ptr` primitive-boundary module under `omega/language/core`
  - initial browsable `Nat::Descending` home for termination ranking docs
  - import canaries for the first core collection/text and pointer primitive
    module surfaces
  - `Slice` core source now exposes index and subslice-style operator contracts
    for start-only, end-only, and bounded range windows
    with proof obligations
  - `Slice::Length` and `Nat::Descending` now have explicit browsable core
    declarations through the current operator declaration surface
  - `Array`, `Vec`, and current text surfaces now expose initial browsable
    length/view operator contracts and primitive boundary providers
  - owner length contracts now use borrowed receivers for `Array`, `Vec`, and
    `String` instead of consuming the owner
  - `Array` and `Vec` now expose browsable index operator contracts with
    bounds proof obligations and primitive boundary providers
  - `Slice`, `Array`, and `Vec` now expose mutable index operator contracts
    with the same bounds proof obligations and distinct primitive boundary providers
  - `Slice` now exposes mutable tail/from/to/range subslice contracts with the
    same bounds proof obligations and distinct primitive boundary providers
  - `StrView` now exposes byte and byte-range view contracts with bounds proof
    obligations and distinct primitive boundary providers
  - `Vec` now exposes an initial allocation-facing `with_capacity` contract
    backed by the existing vector allocation primitive boundary provider
  - `String` now exposes an initial allocation-facing `with_capacity` contract
    backed by a dedicated string allocation primitive boundary provider
  - `String` now exposes capacity and `push_str` mutation contracts so owned
    text growth has a browsable core surface
  - `str` is now a builtin type name, allowing user source to spell borrowed
    text views such as `&str` instead of only core declarations using them
  - bare storage `str` now rejects early with a validation diagnostic directing
    users to `&str`
  - core operators now use `boundary operator` for compiler/runtime-backed
    implementation edges instead of separate root declarations
  - `Ptr` core source now exposes browsable primitive-boundary operator
    contracts for offset, read, write, and pointer-range construction
  Next target:
  - replace comment-only sketches with parser-supported declarations as syntax
    becomes available
  - replace provisional `Array<T>` surface spelling with fixed-length array
    syntax once const-generic array declarations exist
  - decide which names are public core and which are primitive/compiler-only
  - add canaries that import or reference core names directly once syntax exists

- [ ] Private primitive and compiler-handoff layer
  Make the boundary explicit where compiler-managed representation begins.
  Landed:
  - documented the working private carrier model for slice views, string
    views, arrays, vectors, owned strings, and pointer-boundary concepts
  Next target:
  - decide whether `Ptr`, `RawBuffer`, or slice/string descriptors are source-visible primitives or compiler-private concepts
  - turn the documented carrier model into one implementation-owned runtime representation path
  - keep safe source away from raw pointer fields while still giving host/ABI code a truthful low-level model
  - audit places where slice/string descriptor logic is spread across backend stages and identify a single representation owner

- [ ] Boundary primitive registry
  Core contracts need an auditable implementation authority for the private
  compiler/runtime layer.
  Landed:
  - syntax boundary reports now emit target boundary policies, boundary
    contracts, boundary operators, and unchecked policies
  - the pipeline shell includes the Boundary artifact when present
  - core `Slice`, `Array`, `Vec`, `String`, `StrView`, and `Ptr` declare their
    compiler/runtime-backed edges with `boundary operator`
  - imported libraries, capability contracts, and target policies use
    `boundary` clauses consistently
  - the old top-level boundary-root declaration surface was removed
  - the legacy authority syntax, docs, canaries, and report names were removed
  Next target:
  - define the language-authored registry shape for compiler/runtime primitive
    providers such as slice indexing, pointer offset, descriptor construction,
    allocation, and host ABI calls
  - decide whether the registry is package/target metadata, a restricted core
    declaration form, or emitted compiler inventory
  - require boundary implementation bindings to reference registered providers
    once binding syntax exists
  - reject unregistered boundary provider names outside explicitly whitelisted
    toolchain/core packages
  - add canaries for accepted core primitive bindings and rejected unregistered
    bindings once the registry syntax is selected

- [ ] Operator declarations and overload resolution
  Operators should have visible semantic homes instead of being anonymous parser/backend special cases.
  Landed:
  - root-level operator declarations parse as inert declaration surface
  - compile canary for a core-style operator contract signature
  - operator declaration placeholders are preserved through symbol-resolved and
    typed trees
  - operator declarations now carry name path, type parameters, parameters,
    return type, and contracts through typed trees
  - root and domain-owned operator declarations now receive symbols through
    resolved and typed trees
  - root operator declarations are now first-class definitions in the early
    name-resolution report and source symbol table
  - root operator declarations are now first-class declarations in the early
    type-surface report
  - domain-owned operator declarations are now surfaced in early name and type
    reports
  - operator contracts now contribute to the early proof-surface report
  - duplicate root operator declarations reject with a focused diagnostic
  - root and domain operators with the same name but distinct parameter
    signatures are now accepted as overload-set declarations
  - root operator declarations that differ only by return type still reject as
    duplicate ambiguous call signatures
  - generic root operator duplicate checks now alpha-normalize type parameter
    names, so `<T>` and `<U>` cannot bypass duplicate signature detection
  - generic root operator duplicate checks now canonicalize type parameters by
    first use, so reordered generic parameter declarations cannot create
    ambiguous duplicate candidates
  Next target:
  - use operator symbols during overload resolution and validate ambiguous
    operator declarations by signature and context
  - design a declaration form for fixed operator spellings such as `+`, `[]`, and range slicing
  - model `items[index]` and `items[1..]` as core `Slice`/`Array`/`Vec` operator contracts
  - design boundary implementation bindings for core operators without hiding their signatures and proof obligations
  - decide how ordinary trait-like operator requirements relate to existing `trait` machine requirements
  - add ambiguity diagnostics before adding broad overload power

- [ ] Domain-specific operator overloads
  Domain facts should be able to participate in operator resolution when the meaning is unique.
  Landed:
  - domain bodies can contain inert operator declaration surface without being
    mistaken for proof facts
  - compile canary for a domain-scoped operator signature
  - domain operator declarations are preserved as domain-owned declarations
    through typed trees
  - domain operator declarations appear as domain-owned entries in early
    semantic reports
  - duplicate domain operator declarations reject with a focused diagnostic
  - domain operators with the same name but distinct parameter signatures are
    accepted as overload-set declarations
  - domain operator declarations that differ only by return type still reject
    as duplicate ambiguous call signatures
  - generic domain operator duplicate checks now alpha-normalize type parameter
    names, so `<T>` and `<U>` cannot bypass duplicate signature detection
  - generic domain operator duplicate checks now canonicalize type parameters
    by first use, so reordered generic parameter declarations cannot create
    ambiguous duplicate candidates
  Next target:
  - define the first semantic domain-operator representation
  - validate ambiguous domain operator candidates by signature, receiver, and
    proof context
  - prove that only facts in the current context can select domain operator meanings
  - reject ambiguous domain-provided operator candidates
  - keep dispatch compile-time only, with no hidden runtime domain tags
  - add canaries for both successful domain-selected operators and ambiguity errors

- [ ] Measures, orderings, and rankings
  Termination and richer proofs need named well-founded views, not ad hoc checker strings.
  Current prototype:
  - `terminates { ... }` parses and checks for direct recursive shapes
  - `decreases value -> Nat::Descending` works for countdown and bounded-distance shapes
  - `decreases entries -> Slice::Length` is accepted by the termination
    checker for the first shrinking-subslice self-loop shape
  - non-empty slice guards now prove the shrinking tail subslice used by the
    `Slice::Length` termination canary
  - runtime dispatch guard emission now handles guarded termination edges, so
    the shrinking-slice `Slice::Length` canary compiles end-to-end
  - checker recognition of builtin ranking names is isolated behind a small
    internal ranking-order model
  - `Nat::Descending` and `Slice::Length` have temporary core declaration homes
    via operator declarations
  - custom ranking names must now resolve to declared operator-like ranking
    views returning `usize`; undeclared names reject instead of being accepted
    as magic strings
  - custom ranking declarations now have canary coverage for unknown names,
    non-`usize` returns, wrong arity, and parameter/decreases type mismatch
  - `decreases power -> Card::PowerOrder` now compiles for the first declared
    custom natural-ranking countdown shape
  - `decreases card.power -> Card::PowerOrder` now proves the first
    user-defined struct field countdown shape, with a stalled-field fail canary
  Current pending canary:
  - `canaries/pending/termination/custom_ranking_struct_view_unimplemented`
    captures the desired future `decreases card -> Card::PowerOrder` projection
  Next target:
  - replace temporary operator-like ranking declarations with dedicated ranking
    or measure declaration syntax once selected
  - decide how order/measure declarations represent "rank this value by this view"
  - support builtin/default inference for plain `decreases value` only when unambiguous
  - replace arithmetic-facing proof UX such as `limit - index` with named bounded-distance rankings
  - add lexicographic ranking support
  - support multiple named orders for the same data shape
  - extend custom ranking projections/orders from explicit field expressions to
    full user-defined struct views such as `decreases card -> Card::PowerOrder`
  - broaden termination checking beyond narrow direct self-recursion toward SCC/cycle reasoning
  - add a runtime exit canary for shrinking-slice recursion once runtime
    dispatch reliably executes the descriptor update instead of hanging
  - teach runtime value lowering for machine-call results such as
    `let result = self.walk(view)`

## Data, Ranges, And Collections

- [ ] Ranges as proof-backed operators
  Range syntax now exists through syntax, resolved, typed, and checked trees,
  but bounds validity is not generally enforced.
  Landed:
  - obvious literal subslice ranges over fixed-array-derived slice views now
    reject when outside the proven slice length
  - literal start-only, end-only, and bounded subslice forms now have pass/fail
    canary coverage
  - dynamic subslice bounds such as `view[self.start..]`,
    `view[..self.end]`, and `view[self.start..self.end]` now reject until
    the compiler has a real proof that the bound is within the current view
  - local literal integer facts now prove equivalent subslice bounds, covering
    `let start: usize = 1; view[start..]`
  - local integer facts now fold simple integer arithmetic for range proofs,
    covering bounds such as `let end: usize = start + width`
  - obvious literal indexes over fixed-array-derived slice views now reject
    when outside the proven length
  - local literal integer facts now prove equivalent fixed indexes, covering
    `let index: usize = 1; view[index]`, and reject if the fact is out of range
  - folded local integer index facts now prove or reject fixed indexes,
    covering `let index: usize = 3 + 6; view[index]`
  - direct local assignment now refreshes range-checker facts so stale local
    index/range facts cannot prove later slice access
  - obvious literal indexes over fixed-array locals now use the same length
    proof path and reject when outside bounds
  - dynamic indexes and ranges over slice parameters whose current length is
    unknown now reject instead of compiling unchecked
  - direct state-call arguments now seed conservative parameter length/integer
    facts, so `choose(view[1..3], 1)` can prove the callee's dynamic index
  - literal indexes over slice parameters whose current length is unknown now
    reject unless another fact proves the slice has enough elements
  - literal ranges over slice parameters whose current length is unknown now
    reject unless another fact proves the requested window
  - simple guarded transition facts of the form `index < entries.len` now prove
    `entries[index]` on that guarded target path
  - the same `< entries.len` guard now proves start-only subslice bounds such
    as `entries[start..]` on the guarded target path
  - `< entries.len` guards also prove end-only subslice bounds such as
    `entries[..end]` on the guarded target path
  - guarded bounded subslices now require both `end < entries.len` and
    `start <= end`, covering `entries[start..end]`
  - guarded subslices now accept `end == entries.len` as a valid exclusive
    upper bound without treating it as a valid element index
  - range-bound facts are now distinct from element-index facts, so
    `start == entries.len` proves an empty tail slice but not `entries[start]`
  - machine `requires` boolean guards now seed the same range/index facts as
    transition guards, covering slice parameter indexes and bounded windows
  - non-empty guards such as `entries.len > 0` now prove `entries[0]` and
    `entries[1..]`, matching the slice-shrinking loop pattern
  - minimum-length guards such as `entries.len >= 2` and `2 <= entries.len`
    now prove literal indexes and tail subslices within that known prefix
  - nonzero-length guards such as `entries.len != 0` and `0 != entries.len`
    now prove the same non-empty index and tail-subslice facts
  - false branches of `entries.len == 0` now prove the same non-empty slice
    facts for head/tail access
  - boolean equality guard facts now work symmetrically for `condition == true`
    and `true == condition` spellings
  - internal transition continuation range checks now receive negated guard
    facts instead of incorrectly reusing true-branch facts
  - machine-owned integer field initializers now seed range facts for field
    indexes, and direct reassignment refreshes those facts
  - dynamic machine-owned indexed mutable-call runtime coverage now compiles
    and runs, proving machine field index initializers can support mutable
    indexed call arguments
  - `requires room.exit_count < room.exits.len` now survives local aliasing
    through `let exits = room.exits.as_mut_slice()` and
    `let index = room.exit_count`
  - indexed field read/modify/write through an unknown-length slice parameter
    now compiles when `requires index < exits.len` proves the index
  - call-site contract checking can now structurally prove obvious literal
    field/count facts such as `rooms[0].exit_count < rooms[0].exits.len`,
    promoting append-style storage mutation coverage to runtime pass
  - successor guards such as `index + 1 <= entries.len` now prove
    `entries[index]` and `entries[index..]`
  - machine `requires` clauses now cover the same successor-bound slice index
    proof path
  - unknown-length subslice diagnostics now distinguish subslice range proof
    failures from element-index proof failures
  - subslice diagnostics now identify whether the missing proof is the start
    bound, end bound, or start/end ordering
  Next target:
  - thread those refined subslice diagnostics through future operator-contract
    errors once `Slice::from/to/range` contracts drive checking directly
  - broaden state-argument fact propagation to recursive/cyclic control-flow
    paths instead of the current conservative direct-call/transition seed pass
  - extend guard facts into state-call argument propagation for recursive and
    cyclic control-flow paths
  - decide how inclusive/exclusive range forms spell and lower
  - connect range validity facts to indexing validity facts instead of duplicating proof logic

- [ ] Slice runtime descriptor semantics
  Proof and syntax are now ahead of runtime for subslices.
  Landed:
  - literal subslices over fixed-array-backed `as_slice` views now materialize a
    shortened runtime descriptor length, covering `view[1..].len`
  - literal fixed-index reads through literal subslice aliases now copy from the
    adjusted backing array offset, covering `tail[0]` after `view[1..]`
  - state-call argument materialization now resolves prior local slice aliases
    before writing descriptors, covering dynamic `tail[index]` reads after
    `view[1..]`
  - bounded literal subslices now have runtime descriptor coverage through a
    state-call parameter, covering dynamic reads after `view[1..3]`
  - end-only literal subslices now have runtime descriptor coverage through a
    state-call parameter, covering dynamic reads after `view[..2]`
  - literal subslice descriptors now compose nested fixed-array-backed windows,
    covering dynamic reads after `view[1..][..2]`
  - fixed-index local copies now compose nested fixed-array-backed windows,
    covering `let chosen = view[1..][..2][1]`
  Next target:
  - generalize subslice descriptor pointer offsets beyond the fixed-array alias
    copy special case
  - generalize start-only/end-only/bounded descriptors beyond literal fixed-array-backed views
  - ensure descriptor writes/reads have one clear backend representation path
  - promote pending subslice canaries to pass/fail suites when fixed

- [ ] Slice proof vocabulary
  Slices should become a first-class proof object, not just a runtime descriptor.
  Next target:
  - represent non-empty facts, length facts, and window-shrinking facts explicitly
  - prove `items[0]` from non-empty views
  - prove `items[1..]` is shorter under `items.len > 0`
  - carry prefix/suffix/window facts across transitions
  - ensure alias and borrow facts understand subslice overlap conservatively

- [ ] Array and Vec integration
  `Array` and `Vec` should be owners that can produce `Slice` views.
  Next target:
  - make fixed arrays visible as `Array[T; N]` or an equivalent core concept
  - define `Array::as_slice` / `Array::as_mut_slice` as visible operator/machine contracts backed by boundary primitive lowering where needed
  - design `Vec[T]` as owned dynamic storage with length and capacity
  - define how `Vec` borrowing prevents reallocation or mutation that would invalidate active slices
  - add first `Vec` allocation/storage canaries once allocator support exists

- [ ] Str and StrView direction
  Text should follow the same owner/view split as collections.
  Next target:
  - decide whether current `String` remains the public owned text name or evolves toward `Str`
  - define `StrView` or equivalent borrowed text view semantics
  - decide whether string views are byte slices with text domains or their own core view type
  - expose string/text measures and domains such as length, non-empty, UTF-8, and no-NUL from a browsable core surface
  - connect runtime text builders and string comparisons to this semantic model

## Proof And Domains

- [ ] Domains as reusable semantic states
  The executable domain surface is now much healthier.
  Next target:
  - keep strengthening domain fact preservation and invalidation for nested/indexed places
  - add richer sequence/window facts once ranges and slices have proper proof objects
  - decide how domains expose operator meanings without becoming runtime tags
  - keep domain unions/intersections executable only when their bodies are runtime-checkable

- [ ] Proof-checking depth
  Current coverage is broad for calls, exits, mutations, indexing, and boolean implications, but still mostly first-order and local.
  Next target:
  - quantified or sequence-style facts for text/slice invariants
  - reusable proof lemmas for length, bounds, and window transformations
  - better diagnostics when a proof-backed operator is missing a required fact
  - boundary proof boundaries for host/core primitive implementations

- [ ] Borrow checking over views
  Slice and string views make overlap reasoning central.
  Landed:
  - immutable view-producing helpers such as `as_slice` and `as_view` now
    create local borrow loans, so owner mutation before the view's last use
    rejects for both slice and string views
  - derived subslice aliases now rebase their borrow loan through the source
    view, so owner mutation under a live `tail = view[1..]` alias rejects
  - fixed integer indexes are now checked against fixed subslice-range loan
    segments, allowing provably disjoint owner writes such as index `0` while
    `view[1..]` remains live
  - mutable element reborrows can now be derived from a live mutable slice
    parent, while same-index and unknown-index mutable reborrow conflicts stay
    rejected
  - borrow diagnostics now print indexed places with source expression labels
    instead of internal expression arena indexes
  Next target:
  - distinguish more disjoint fixed windows and bounded range/range cases where provable
  - ensure `Vec` mutation/reallocation is rejected while borrowed views exist
  - add canaries for array, slice, and future vec aliasing cases

## Runtime And Backend

- [ ] Runtime text and IO confidence
  Runtime string/text support is real but still ahead of the final semantic model.
  Next target:
  - multi-step text flows with richer transitions
  - host/runtime confidence around real console interaction paths
  - align text storage, text builders, and future `Str`/`StrView` semantics

- [ ] Persistent machine/state mutation confidence
  Writes in one state should reliably be observable in later states and transitions.
  Next target:
  - broader multi-edge/full-package flows
  - generator-style nested storage updates
  - dungeon-sample blockers rather than only isolated micro-shapes

- [ ] Backend representation ownership
  A lot of slice/string/storage behavior is encoded across selection, runtime storage, state values, and ISA lowering.
  Landed:
  - split string literal mutation and text comparison selection out of the
    general runtime mutation writer into a focused string write module
  Next target:
  - identify one representation model for fat descriptors and pointer-based carriers
  - reduce duplicate descriptor assumptions across backend crates
  - keep backend reports explicit about descriptor writes, pointer offsets, and lengths
  - add focused pending canaries before turning any runtime gap into a false pass

- [ ] Strengthen assigned-target allocation
  Evolve the current assigned-home model into a more mature register/stack allocation story with clearer register classes, spill behavior, and post-assignment cleanup.

- [ ] Reduce host/runtime special-case lowering
  Keep shrinking bring-up-era special handling around stdin/stdout/process calls so host/runtime lowering feels like a real subsystem instead of a narrow happy path.

## Canaries And Tooling

- [ ] Maintain three honest canary categories
  `pass` means supported, `fail` means rejected as intended, and `pending` means the desired language behavior is known but implementation is still behind.
  Landed:
  - pending canaries can now track both currently-rejecting future passes and
    currently-accepting future rejects
  Current pending canaries:
  - `canaries/pending/termination/custom_ranking_struct_view_unimplemented`
    should become a pass when ranking views can project through declared bodies
  Next target:
  - promote pending canaries quickly when fixed
  - add pending canaries for serious known gaps instead of leaving them as ad hoc run probes
  - keep fail canaries focused on intended diagnostics
  - avoid letting compile-only pass canaries imply runtime support

- [ ] Language guide and core docs
  The guide now captures the high-level direction, but the actual core source surface is still missing.
  Next target:
  - add a dedicated guide section or chapter for core semantic types once syntax stabilizes
  - keep traits/modules/host-boundaries sequencing coherent
  - add navigable core docs alongside `omega/language/core` once declarations exist
  - keep speculative topics clearly labeled as working direction
