# Tasks

Compiler/runtime work surfaced by the canary ladder, the dungeon sample, and
the current language-design push.

The major direction is now clearer: Omega needs a small set of browsable core
semantic concepts, plus a deliberately private compiler/runtime layer where
the unsafe representation work happens. Users should be able to navigate to
`Slice::Length` or an indexing operator contract and understand the language
meaning, without needing access to pointer descriptor internals.

## Architecture Tracks

- [ ] Pipeline semantic ownership boundaries
  Keep pipeline stages organized around the nouns they truly own: places,
  values, facts, loans, moves, drops, calls, transitions, effects, and boundary
  edges.
  Landed:
  - typed-to-checked range validation now consumes proven scalar upper-bound
    facts such as `requires index < array_length` for known-length indexing,
    so dynamic indexed contract preservation can reach contract proof instead
    of failing early in range validation
  - pipeline architecture docs now define the ownership rule for when a stage
    owns a semantic noun versus merely preserving or lowering it
  - pipeline architecture docs now include a central semantic ownership matrix
    across all durable pipeline stages
  - the syntax-to-symbol-resolved stage doc now explicitly says it owns symbol
    identity only and must not typecheck, prove, borrow-check, or schedule flow
  - checked-flow, state-graph, and control-flow representations now have
    arena-backed move/drop ownership event plumbing with state-local spans
  - checked-tree-to-state-graph and state-graph-to-control-flow preserve
    ownership summaries without nested per-state allocations
  - checked-flow now emits initial conservative move/drop events for path-like
    assignment/local-initializer moves and state-exit local drops
  - checked-flow now emits conservative call-source move events for direct
    by-value machine call arguments while skipping reference parameters
  - checked-flow ownership events now skip copy-like scalar locals and
    by-value scalar call parameters instead of treating every place-like scalar
    read as an ownership move/drop
  - checked-flow assignment ownership events now resolve RHS place types through
    state parameters, local declarations, machine data, attached data fields,
    and indexed elements so copy-like scalar assignments do not materialize
    fake move/drop facts
  - checked-flow call ownership events now recover arguments through the shared
    semantic call-site lookup, so transition target arguments materialize
    ownership moves instead of being invisible to the move/drop spine
  - the ownership canary set now covers nested expression-call arguments,
    proving the shared call-site lookup also materializes ownership moves for
    by-value arguments inside nested state calls
  - checked-flow ownership event production now descends into array and struct
    literal initializers/call arguments so owned places nested inside aggregate
    construction produce move facts
  - checked-flow ownership production is now split into entrypoint, move
    traversal, call argument routing, state-exit drops, move/drop fact emission,
    contextual place-type resolution, and type-reference ownership
    classification modules instead of concentrating every ownership concern in
    one file
  - checked-flow place handling now splits generic canonical-place conversion
    from state-local contextual name/member recovery, so place construction,
    comparison, and type/member resolution have distinct module seams
  - typed-to-checked borrow access collection now splits borrow-access place
    construction from state-local contextual name/member resolution
  - typed-to-checked borrow access collection now splits argument-access fact
    emission out of read/mutable access traversal
  - typed-to-checked borrow call discovery now threads recursive expression and
    transition traversal through a named `BorrowCallCollection` context instead
    of raw arena/ordinal/state parameters
  - typed-to-checked borrow call discovery now keeps the shared borrow-call
    arena/ordinal context in its own module instead of the statement router
  - typed-to-checked borrow fact production now splits state-local writable
    root, loan, call-access, and last-use assembly out of the program-level
    borrow entrypoint
  - checked-flow move traversal now descends into binary/range value
    expressions, with a string-concat ownership canary proving owned operands
    are materialized as move facts
  - checked value fact production is now split into entrypoint, statement-role
    routing, transition target value routing, and nested expression traversal
    modules instead of concentrating all value concerns in one file
  - checked value facts now cover machine decreases and attached-data field
    initializers, with focused canaries for both top-level value origins
  - range checking now keeps its root as an entrypoint, with initializer fact
    seeding and statement/transition range routing split into focused modules
  - guard-derived range facts now split boolean guard dispatch from
    comparison-derived length/index/order bound seeding
  - range state-argument fact propagation now splits the merged parameter fact
    model from statement/transition traversal
  - contract checking now keeps its root as an entrypoint, with call
    `requires` validation and exit `ensures` validation split into focused
    modules
  - contract label helpers now split call-site argument substitution from
    domain proof label substitution
  - checked-tree-to-state-graph contract preservation now splits state-local
    contract summary construction from worker-local graph remapping
  - checked-tree-to-state-graph borrow preservation now splits call argument
    access summaries from active-loan, activation, and weakening summaries
  - checked-tree-to-state-graph ownership preservation now splits state-local
    move/drop preservation from worker-local ownership arena remapping
  - checked-tree-to-state-graph segment scheduling now splits segment-transition
    payload helpers out of the statement-to-segment splitter
  - checked-tree-to-state-graph worker merge now bundles worker-local source
    arenas so state remapping reads as one semantic source bundle
  - state-graph-to-control-flow now has focused unit canaries for preserving
    ownership summary spans and owned move-event call sources across the handoff
  - state-graph-to-control-flow now splits borrowed and owned graph remapping
    out of the builder facade
  - abstract-to-target lowering now has focused unit coverage proving ownership
    summaries survive target-operation legalization beside values and boundaries
  - abstract-to-target lowering now splits instruction legalization, instruction
    operand translation, runtime value translation, and metadata preservation
    tests out of the translator root
  - target-to-assigned lowering now has focused unit coverage proving ownership
    summaries survive physical-home assignment beside value summaries
  - target-to-assigned lowering now splits function remapping, operation and
    operand conversion, runtime value home assignment, and metadata preservation
    tests out of the builder root
  - assigned-to-machine-instructions now has focused unit coverage proving
    value, boundary-edge, and ownership summaries survive symbolic instruction
    emission
  - assigned-to-machine-instructions now splits function-local instruction
    appends and metadata preservation tests out of the builder root
  - target-to-machine-program now has focused unit coverage proving value,
    boundary-edge, and ownership summaries survive the aggregate bridge into
    the current machine-program artifact
  - object-file planning now splits entry machine layout and encoded entry
    function lookup out of object-plan orchestration
  - object-file planning tests now live outside the orchestration module so
    `builder.rs` stays focused on the stage conveyor
  - final-image construction now splits object symbol/import/relocation copying
    and object section lookup out of the builder root
  - final-image construction tests now live outside the builder root so
    `builder.rs` stays focused on conversion orchestration
  - final-image relocation application now splits checked text-section byte
    patching out of architecture relocation math, with a canary for
    out-of-bounds relocation patches
  - direct image emission now splits input DTOs, target support facts, writer
    dispatch, and planned-vs-encoded byte validation out of the crate root
  - control-flow-to-abstract-operations now preserves control-flow move/drop
    events into an arena-backed abstract ownership summary
  - abstract-to-target and target-to-assigned lowering now preserve ownership
    summaries instead of silently dropping them at backend representation seams
  - assigned-to-machine-instructions and the current machine-program aggregate
    now preserve ownership summaries as metadata
  - control-flow-to-abstract-operations now records lowered host operations as
    abstract boundary-edge summaries, and downstream backend representations
    preserve those summaries as metadata
  - control-flow-to-abstract-operations now splits the transitional
    instruction-selection input DTO from abstract summary attachment
  - typed-to-checked now records checked-flow boundary edges for calls into
    states supplied by boundary trait signatures
  - typed-to-checked flow construction now splits boundary-edge discovery
    through boundary trait conformances out of call flow fact construction
  - typed-to-checked call flow construction now splits entry, requires,
    invalidation, and exit context phases out of call fact assembly
  - checked boundary edges now preserve through state graph and control flow as
    state-local boundary summaries
  - control-flow-to-abstract-operations now preserves source-level boundary
    edges beside lowered host-operation boundary edges in abstract summaries
  - `omega-facts` is now split around fact model definitions, fact-plan arena
    storage/query helpers, context views, place resolution, definition fact
    extraction, and tests instead of hiding every fact concern in `lib.rs`
  - symbol-resolved-to-typed expression lowering now keeps `expression.rs` as
    the entry surface while recursive table lowering and expression lowering
    canaries live in focused sibling modules
  - `omega-checked-trees` now has noun-shaped representation modules for
    borrow facts, proof facts, invariant/domain facts, checked flow facts,
    checked value facts, statement wrappers, and the root checked-tree container
  - typed-to-checked now builds initial checked value facts for decreases
    clauses, initializers, statement values, call arguments, transition
    guards/targets, and nested expression children
  - checked value facts now preserve through state graph and control flow as
    state-local value summaries
  - control-flow value summaries now preserve into abstract operations as an
    abstract value summary beside ownership and boundary summaries
  - abstract value summaries now preserve through target-operation legalization
    as target value metadata
  - target value summaries now preserve through assignment as assigned target
    value metadata beside ownership and boundary summaries
  - assigned value summaries now preserve through symbolic machine
    instructions and the current machine-program artifact as metadata
  - encoded machine bytes now preserve value, boundary-edge, and ownership
    summaries from symbolic machine instructions
  - `omega-state-graph` and `omega-control-flow` now mirror that representation
    shape with modules for topology, graph/plan containers, contracts, proof
    facts, invariants, borrow facts, value facts, ownership facts, operations,
    and transitions
  - `omega-assigned-target-operations` now has a thin crate root and
    noun-shaped modules for assigned operands, value homes, operation kinds,
    operation conversions, operation records, function metadata, plan storage,
    and runtime-value source behavior
  - `omega-machine-instructions` now has a thin crate root and noun-shaped
    modules for symbolic instruction records, function metadata, plan storage,
    and machine-program conversions
  - `omega-machine-program` now has a thin crate root and noun-shaped modules
    for artifact instruction records, function metadata, and plan storage
  - `omega-machine-bytes` now has a thin crate root and noun-shaped modules
    for encoded function ranges, encoded instruction byte spans, and encoded
    plan storage
  - `omega-object-file` now has a thin crate root and noun-shaped modules for
    object plans, sections, symbols, relocations, naming, and container
    serialization
  - `omega-object-file-planning` now has a thin crate root plus modules for
    planning input, build orchestration, section sizing, and symbol planning
  - object-file planning now has a focused unit canary for text/data/bss
    section sizing, runtime-frame bss placement, entry/import symbols, and
    data-object symbol emission
  - object-file planning now has focused unit canaries for missing
    entry-machine layout and missing encoded entry-function diagnostics
  - relocation instruction-record extraction now routes through a thin module
    entrypoint with runtime-value, runtime-text, and runtime-storage relocation
    families split out of the dispatch table; runtime text is split into
    append, compare, materialize, read, and write relocation families, while
    runtime storage is split into address, compare, copy, string, and write
    relocation families
  - relocation offset math now has a thin module entrypoint with separate
    external-call, runtime-frame-index, runtime-storage, and runtime-text
    offset families
  - relocation planning now has a thin crate root plus focused modules for
    planning input, per-function walking, and target-specific data-address
    relocation record facts
  - data-address relocation planning now has a focused canary for AArch64 page
    relocations and x86_64 absolute-address relocations
  - data-address relocation planning now separates assigned-operand scanning
    from target-specific relocation record insertion
  - data-address relocation offset math now lives with relocation offset
    families and has a focused canary for architecture-specific prior operand
    widths
  - runtime-storage relocation offset math now splits compare, copy,
    string-descriptor, and write/binary operand offsets into focused modules
  - runtime-text relocation offset math now splits append, materialize, and
    host-backed line-read offsets into focused modules
  - host-operation relocation routing now owns both data-address operand
    relocation scanning and external import call relocation insertion, leaving
    the instruction-record router as family dispatch
  - runtime-storage write relocation planning now routes operand-start and
    runtime-frame address offsets through the relocation offset family, with a
    canary for architecture-specific write offsets
  - compatibility object-container serialization now splits primitive byte
    writing and stable enum ID mapping out of the orchestration module, with a
    canary for the object-container IDs
  - compatibility object-container serialization now splits section summary,
    symbol metadata, and relocation metadata serialization out of the
    orchestration module, with a canary for BSS summary sizing
  - `omega-image` now has a thin crate root plus modules for final-image model
    records, object/relocation conversion, symbol/address queries, emitted
    output records, and architecture relocation patching
  - final-image construction now has a focused unit canary for preserving
    object symbols, imports, bss sizing/alignment, relocation symbol handles,
    and final symbol address lookup
  - `omega-image-pe` now splits PE constants, byte writers, alignment helpers,
    header/section writing, and import thunk/table construction out of the
    executable-emission root
  - PE direct image emission now splits section/RVA/raw-offset planning into a
    focused module with a canary for data and BSS layout facts
  - `omega-image-macho` now splits import thunk installation, bind-info
    construction, and AArch64 thunk patching out of the executable-emission root
  - Mach-O direct image emission now splits command sizing, section/data
    segment placement, bind offsets, linkedit offsets, and code-signature
    planning into a focused module with a final layout canary
  - `omega-image-elf` now splits ELF constants, byte writers, alignment
    helpers, and ELF/program-header writing out of the executable-emission root
  - ELF direct image emission now splits text/data/BSS address planning into a
    focused module with a canary for final layout facts
  - ELF, PE, and Mach-O direct image emission now split entry-symbol lookup into
    focused modules with canaries for final-image entry address/offset/RVA
    resolution
  - ELF direct executable emission now uses the final-image entry symbol offset
    instead of assuming the entry point is the first byte of `.text`
  - `omega-validation` now has a thin validation entrypoint plus semantic
    modules for tests, runtime entry-point checks, local writable roots,
    assignment places, calls, transitions, proof facts, domain membership,
    invariants, data shapes/types, machine data, expression types,
    type-reference validation, state signatures, trait requirements,
    trait conformance, and top-level versus machine-local symbol scopes
  - `omega-backend-report` now separates backend report input/timing DTOs from
    the large report text writer root while preserving the public report API
  - `omega-backend-report` now splits state-call lowering and alias-flow report
    sections out of the root writer into a focused report-section module
  - `omega-backend-report` now splits state/runtime storage, state values, and
    runtime text report sections out of the root writer into focused modules
  - `omega-backend-report` now splits the source native surface report section
    out of the root writer into a focused module
  - `omega-backend-report` now splits state schedule reporting out of the root
    writer into a focused module
  - `omega-backend-report` now splits runtime state-flow and dispatch reporting
    out of the root writer into a focused transition/report module
  Next target:
  - normalize the remaining stage pages to use the same compact ownership table
    format where prose is currently vague
  - make ownership event production fully type-aware so Copy/no-drop values and
    real ownership-consuming values are distinguished across all transfer sites
  - extend type-aware ownership event production into slice/string operators
    beyond binary expressions and future user-defined copy/drop policy
  - teach remaining value-expression analysis to append ownership
    transfer/drop events into the existing checked-flow ownership arenas
  - lower abstract ownership summaries into explicit backend transfer and
    cleanup operations
  - refine checked/control-flow/abstract/target value summaries with type-aware
    ownership kind, drop policy, storage consequences, and backend lowering
    beyond metadata so machine-instruction/object layers can lower value
    consequences deliberately instead of only preserving summaries
  - link abstract source boundary edges to lowered host-operation boundary
    edges and validate the pair against target policies
  - continue splitting relocation planning where remaining files mix data,
    instruction, symbol, and target-offset responsibilities
  - keep relocation offset helpers paired with the relocation families that
    consume them; do not let `offsets.rs` grow back into a cross-family bag of
    byte constants
  - keep shrinking image writer responsibilities where crate roots still mix
    high-level layout orchestration and output assembly
  - link final-image imports/fixups back to source and lowered boundary-edge
    summaries for reporting and target-policy validation
  - consider breaking `omega-assigned-target-operations` operation conversions
    into semantic families if the conversion table keeps growing
  - continue splitting `omega-validation` where files still mix semantic
    families, especially type-reference validation, state-signature validation,
    and effect diagnostics
  - continue splitting `omega-backend-report` into report-section modules so
    call lowering, storage, runtime text, scheduling, dispatch, proof, object,
    and codegen reporting do not live in one root
  - keep splitting overloaded pipeline files when a helper starts owning a noun
    outside that stage's documented responsibility

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
  - imported libraries, authority contracts, and target policies use
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
  - repair dynamic indexed domain-fact preservation across disjoint mutating
    calls; the current unit baseline still rejects some `self.index` proofs
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
  - repair direct owner assignment rejection while a local borrow alias remains
    active; the current unit baseline has this borrow diagnostic expectation red
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
