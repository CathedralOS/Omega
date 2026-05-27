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
  - import canaries for the first core collection/text module surfaces
  - `Slice` core source now exposes index and subslice-style operator contracts
    with proof obligations
  - `Slice::Length` and `Nat::Descending` now have explicit browsable core
    declarations through the current operator declaration surface
  Next target:
  - replace comment-only sketches with parser-supported declarations as syntax
    becomes available
  - decide which names are public core and which are primitive/compiler-only
  - add canaries that import or reference core names directly once syntax exists

- [ ] Private primitive and compiler-handoff layer
  Make the boundary explicit where compiler-managed representation begins.
  Next target:
  - decide whether `Ptr`, `RawBuffer`, or slice/string descriptors are source-visible primitives or compiler-private concepts
  - document the runtime carrier shapes for slice views, string views, arrays, and vectors
  - keep safe source away from raw pointer fields while still giving host/ABI code a truthful low-level model
  - audit places where slice/string descriptor logic is spread across backend stages and identify a single representation owner

- [ ] Trusted primitive registry
  Core contracts need an auditable implementation authority without inventing ad hoc keywords on every declaration.
  Landed:
  - syntax trust reports now emit trust roots, target trust policies, trusted contracts, unresolved trusts, and unchecked policies
  - the pipeline shell includes the Trust artifact when present
  - core `Slice` declares the first language-authored primitive trust roots for indexing, subslicing, and length ranking
  - core `Ptr` declares initial primitive trust roots for offset, read, write, and pointer-range operations
  - trust definitions now receive dedicated `Trust` symbols in the early name
    surface instead of being hidden as generic objects
  - trust artifacts now show checked and unchecked reference counts per trust
    root
  Next target:
  - define a language-authored registry for compiler/runtime primitive roots such as slice indexing, pointer offset, descriptor construction, allocation, and host ABI calls
  - require trusted implementation bindings to reference registered roots
  - promote trust-root usage counts into stricter registry validation once
    implementation-binding syntax exists
  - reject unregistered trusted implementation names outside explicitly whitelisted toolchain/core packages
  - add canaries for both accepted core primitive bindings and rejected unregistered bindings once the syntax is selected

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
  Next target:
  - use operator symbols during overload resolution and validate ambiguous
    operator declarations by signature and context
  - design a declaration form for fixed operator spellings such as `+`, `[]`, and range slicing
  - model `items[index]` and `items[1..]` as core `Slice`/`Array`/`Vec` operator contracts
  - design trusted implementation bindings for core operators without hiding their signatures and proof obligations
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
  - `decreases entries -> Slice::Length` works for the first shrinking-subslice self-loop shape
  - checker recognition of builtin ranking names is isolated behind a small
    internal ranking-order model
  - `Nat::Descending` and `Slice::Length` have temporary core declaration homes
    via operator declarations
  Current pending canary:
  - `canaries/pending/termination/custom_ranking_order_unimplemented`
  Next target:
  - replace temporary operator-like ranking declarations with dedicated ranking
    or measure declaration syntax once selected
  - decide how order/measure declarations represent "rank this value by this view"
  - support builtin/default inference for plain `decreases value` only when unambiguous
  - replace arithmetic-facing proof UX such as `limit - index` with named bounded-distance rankings
  - add lexicographic ranking support
  - support multiple named orders for the same data shape
  - add custom ranking projections/orders for user-defined structs
  - broaden termination checking beyond narrow direct self-recursion toward SCC/cycle reasoning

## Data, Ranges, And Collections

- [ ] Ranges as proof-backed operators
  Range syntax now exists through syntax, resolved, typed, and checked trees,
  but bounds validity is not generally enforced.
  Landed:
  - obvious literal subslice ranges over fixed-array-derived slice views now
    reject when outside the proven slice length
  - literal start-only, end-only, and bounded subslice forms now have pass/fail
    canary coverage
  - obvious literal indexes over fixed-array-derived slice views now reject
    when outside the proven slice length
  - obvious literal indexes over fixed-array locals now use the same length
    proof path and reject when outside bounds
  Next target:
  - require proof that dynamic `view[start..]`, `view[..end]`, and
    `view[start..end]` are valid for the current view
  - decide how inclusive/exclusive range forms spell and lower
  - connect range validity facts to indexing validity facts instead of duplicating proof logic

- [ ] Slice runtime descriptor semantics
  Proof and syntax are now ahead of runtime for subslices.
  Current pending gaps:
  - `canaries/pending/slices/runtime_subslice_range_len_wrong`
  Next target:
  - fix `view[1..]` materialization so `tail.len` and pointer offset are correct at runtime
  - support start-only, end-only, and bounded subslice descriptors
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
  - define `Array::as_slice` / `Array::as_mut_slice` as visible operator/machine contracts backed by trusted primitive lowering where needed
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
  - trusted proof boundaries for host/core primitive implementations

- [ ] Borrow checking over views
  Slice and string views make overlap reasoning central.
  Next target:
  - conservatively detect overlap between parent slices and subslices
  - distinguish disjoint fixed windows where provable
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
