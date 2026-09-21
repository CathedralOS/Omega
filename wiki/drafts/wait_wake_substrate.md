# Wait/wake substrate vocabulary (design draft)

Status: design draft only. WAIT-WAKE-SUBSTRATE is scope-verified on the board
with an authorization gate recorded — nothing here is permission to implement.
Activation requires a concrete customer plus a target's real wait mechanism
([chapter_18 §Waitable Contracts](../language_guide/chapter_18_concurrency.md):
"an engineering direction, not permission to describe unlike host mechanisms
as equivalent").

## Anchors

- `wiki/language_guide/chapter_18_concurrency.md` §Waitable Contracts:
  Retained Substrate Direction — "a useful shared substrate is a small
  word/value wait plus wake-one/wake-many boundary"; wait publishes
  suspension/blocking and progress separately; wake does not inherit a
  waiting ceiling merely because it reaches the scheduler; a protocol
  proof must know what can unblock a wait.
- `wiki/spec/build/task_runtime.md` §library-and-foreign-providers — same
  direction, plus "wake need not park" and "wait's suspension/blocking
  and positive progress remain independent explicit premises."
- `source/library/blocking-executor/executor.omg` — the only landed
  declaration of the substrate:

  ```omega
  pub boundary data WaitSubstrate;
  boundary machine WaitSubstrate::park(word: u64) suspends; blocks;
  boundary machine WaitSubstrate::wake_one(word: u64);
  boundary machine WaitSubstrate::wake_all(word: u64);
  ```

## What is settled

- **Shape**: one `boundary data` carrier, three boundary machines. `park`
  is the only suspension site: `suspends; blocks` marks it as a canonical
  suspension crossing that also blocks the underlying execution context.
  `wake_one`/`wake_all` are ordinary boundary calls — neither suspends.
- **Join key**: a `u64` word. The word is deliberately the smallest
  identity a host mechanism can share — a futex word, a parked-address
  key, an event handle slot. The vocabulary does not specify allocation
  or freshness of the word; that is the provider contract's premise.
- **Custody**: the parked continuation is compiler/provider-owned; a
  parked claim cannot settle. `park` returns to the same activation under
  unchanged bindings when a wake lands.

## The gap the vocabulary must close

The settled plan ledger models park/settle/routing only intra-activation:
`TaskActivationPlan.canonical_suspension_crossings` records *where* a
claim may suspend, but nothing records *what unblocks it*.
`composition_model::CompositionCrossActivationEdges` therefore publishes
`NotRetained` — joins, channel handoffs, and every other cross-activation
wait-for relation have no field to live in. The vocabulary a whole-
composition extractor needs, in terms of what already exists:

- **Wait-side binding**: for each `park` crossing, the provenance of its
  `word` operand — literal, forwarded parameter, or computed — so an
  extractor can join a parked crossing to the wake sites that can reach
  the same word.
- **Wake-side enumeration**: for each `wake_one`/`wake_all` call site,
  the same word provenance plus the one-vs-all distinction. A retained
  edge is then (waking site, word, count) → (parked crossing, word).
- **Progress premises published separately**: a wait's suspension and
  its progress ceiling are independent fields, not one boolean. Wake
  sites carry no waiting ceiling; they are edges into the graph, not
  crossings of it.

## Non-goals

- No equivalence claim across host mechanisms. Futex-style word waits,
  event objects, and port sets are distinct provider contracts; the
  vocabulary names what each must publish, not that they are the same.
- No fairness/ordering promise. The spec names wake-one vs wake-many;
  it does not promise FIFO wake order, and the vocabulary must not
  smuggle one in.
- No timeout or cancellation form here. Deadlines and cancellation are
  the task-runtime contract's own premises; a wait word is only the
  unblock channel. Whether a cancellation participates in the same word
  protocol is a customer-visible decision, deferred with the rest.
- No mailboxes or payload transfer. A wake carries no value; the word
  is synchronization identity only. Payload channels are separate
  construction over ordinary custody.

## Open questions for the activating customer

1. **Word equality scope**: is the join key compared structurally (u64
   equality) or does the vocabulary retain a provenance/lineage token so
   two activations computing `word = 7` independently do not join? The
   extracted edge set is only sound if word identity is an allocation,
   not a coincidence.
2. **Wake-before-park**: does the substrate require a registration
   (parked set) or is a wake on an unwaited word a no-op? The landed
   decl implies the latter; a retained plan may need the former to
   reason about lost wakeups.
3. **one-vs-all retention**: `wake_one` wakes an unspecified single
   waiter. For proof purposes the vocabulary likely needs only
   "may unblock some parked crossing on this word" vs "may unblock all",
   but a protocol with mandatory unblock coverage may need a stronger
   premise.
4. **Crossing identity**: parked crossings already have canonical
   identities in `canonical_suspension_crossings`; the edge vocabulary
   can reference those IDs rather than inventing new site handles.

## Activation checklist

Per the spec deferral, extraction activates only with:

- a concrete protocol or safety-profile customer that needs
  cross-activation reasoning;
- a target whose real wait mechanism satisfies the published contract
  (suspension/blocking/progress published separately);
- the upstream inter-activation vocabulary — this document's scope —
  settled enough that `CompositionCrossActivationEdges` can carry the
  joined edges instead of `NotRetained`.
