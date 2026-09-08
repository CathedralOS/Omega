# Extent authority and mappings

An address is data, not authority. Computing or copying an `addr` grants no
right to access storage, install a mapping, execute code, or transfer control.
Hardware operations require checked authority, complete compiler-understood
operation contracts, or explicitly admitted provider commitments with receipts.
Wrappers preserve reach, authority, clobbers, trust, and installed-root obligations.

## Qualified geometry

`Extent` is ordinary linear data with runtime `base: addr` and `length: u64`.
Its core-owned routed `Granted` domain establishes a live authority claim.
Constructing identical fields produces unqualified data, not a usable resource.
Zero-filled storage establishes neither `Granted` nor a linear debt;
`ExtentSlot { Empty | Live(Extent) }` supplies a debt-free empty state.

The transparent geometry obligation is the proposition
`embed(base) + embed(length) <= addr::Bound`, using unbounded proof `Int` and
the sealed selected-target bound. It is not wrapping address arithmetic or an
executable Boolean validator. Its proof cannot establish routed authority.
See [counts and addresses](../language/counts_and_addresses.md).

Every claim retains its address space, rights, stable backing identity, minting
provenance, parent/root lineage, lifetime or mapping era, and ownership.
Physical, virtual, I/O-port, and provider-defined spaces use the same range
algebra but are not interchangeable. Rights are independently established facts.
These facts need not occupy runtime bits while evidence survives every required
crossing. Dynamic lookup or revocation may use an ordinary provider-owned handle.

`ExtentRootProvider::grant` establishes its exact qualified result through the
selected admitted boundary occurrence. A direct checked-adapter call is not that
crossing. The caller supplies geometry; the provider receipt separately admits
stable backing, ownership, and fresh nonduplicating issuance. A fingerprinted
ordinary postcondition bounds each invocation's geometry in the same canonical
interval algebra as the domain's content projection. A result-selected base can
prove size but cannot prove freshness. There is no proof-only receipt binder.

Program image and initial storage use the same core-owned domain through
[installed program entry](../build/entry_roots.md), not target-owned substitutes
or name-based recognition. Both exact `ProgramStorageEntry::enter` positions
must satisfy `Granted::no_wrap` before either complete fact is imported;
rejection returns all moved bootstrap inputs. The physical entry stack is a
separate execution resource, not source-visible storage. Image sections are
borrowed views below the installed image root.

## Conservation and loans

The owner-unique `Content` projection maps `Granted` to normalized half-open
address-space intervals with proof-natural bounds. The one-past endpoint may
equal the address-space bound without being representable as an `addr`.
[Content conservation](content_custody.md) requires:

```text
parent content = separate(all child content)
```

Split consumes the parent and returns disjoint children covering it exactly.
Containment of each child or a scalar length sum is insufficient. Merge proves
the reverse equation over compatible common root lineage; literal siblinghood
is not required. Numeric adjacency alone proves neither compatible rights nor
provenance or era. Joining unrelated grants requires an explicit provider
operation establishing new combined authority. Failed consuming operations
return every input authority.

Attenuation preserves content while permanently discarding rights. Merge cannot
recreate discarded write permission. Authority intended to return remains in a
separate claim or loan. Every access proves its entire touched interval lies
within the claim's projection. Conservative underapproximation restricts use;
an establishment backing smaller than the declared projection rejects.

Subrange loans are ordinary borrow-carrying values, not cleanup debts. Shared
loans permit shared operations; write-only exclusive loans permit non-observing
replacement; read/write exclusive loans permit mutation. Layout projection,
placed fields, and allocator-private free-list geometry do not split owned
authority. A subrange leaving its parent's ownership domain does require a
conserved owned split. [Allocation](allocation.md) remains package behavior.

Exclusive claims over the same stable backing descend from one custody root.
That root delegates separated ranges or explicitly authorizes shared aliases.
Independent provider-local ledgers do not prove cross-provider exclusivity.

Boot handoff appends classified custody-succession edges without rewriting
backing identity or history. Preserved delegations remain honored by the
successor. Reclaimable classes become capacity only after proving no live claim
overlaps them; retained classes stay with the continuing owner; excluded classes
remain unavailable. External classification is admitted, while locally tracked
range checks are derived.

Destroying placed content returns range authority but does not release it to
firmware or an OS. Implicit provider reclamation must be terminating,
infallible, non-suspending, and nonblocking. Otherwise expose explicit terminal
`release`, and only where policy permits permanent loss, `abandon`. Safety
profiles may reject abandonment; silent drop cannot conceal capacity loss.

## Mapping and reclamation

Mapping requires source authority and destination authority independently. A
numeric destination is only a hint. Fixed placement consumes an owned virtual
extent; automatic placement draws from a caller-supplied virtual-space
allocator with that authority. Source custody may be owned, shared-borrowed, or
exclusive-borrowed. The mapped extent retains that relationship; a borrowed
source cannot be reclaimed while it lives.

A mapping grant binds source/destination spaces, required rights, custody,
mapped facts, and all translation-activation and release obligations. Structural
validation creates only a pending mapping, with no mapped access. The exact
provider receipt must establish installed translations and discharge activation
facts before mapped loans exist. Shared source custody cannot expose mutable
mapped loans.

Unmapping retains every authority until an exact provider receipt establishes
that stale translations are released and target completion facts hold. It then
returns the reusable destination and any owned source, or ends the source loan.
Cross-core invalidation may require a linear shootdown/quiescence token before
reuse. Its suspension/blocking ceiling remains visible to callers, including
interrupt roots.

Reclamation requires exclusive ownership back with no live in-language views;
there is no per-access generation probe. An erased era comparison cannot make
asynchronous revocation safe. Forced revocation requires an explicit fallible
provider quiescence/lifecycle protocol, not an implicit property of mappings.

Owned virtual-to-physical decomposition cannot conserve two independent
projections when their correspondence matters. It requires a compact canonical
mapping algebra with decidable containment, restriction, equality, and separated
composition. Until such an algebra is specified, that decomposition rejects;
interval and counted-quantity algebras do not express the correspondence.

Page tables, translation policy, activation, shootdowns, and teardown belong
to OS/provider packages. Omega owns the general authority and checking rules,
not compiler-specific page-table or allocator lifecycle types.
