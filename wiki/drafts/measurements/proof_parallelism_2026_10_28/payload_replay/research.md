# Carrier-index experiment sources

Question: can immutable domain routing metadata remove repeated unrelated-carrier scans without caching proof results?

Repository search: `payload`, `membership replay`, `membership history` under wiki/drafts/measurements. Existing records identify source-contract replay and prior snapshot-sharing work; last-edge moves and shared scalar theories already exist, so they are not new candidates.

Online query: `site.doc.rust-lang.org std collections BTreeMap get entry or_default`. Read the official standard-library page https://doc.rust-lang.org/std/collections/struct.BTreeMap.html (displayed std 1.99.1): ordered-key map, borrowed get, and Entry insertion. These longstanding APIs exist on the pinned toolchain; no new dependency or nightly API is introduced. The documentation establishes API behavior, not a performance win.

Safety argument: for a fixed carrier c, the bucket is the original catalog subsequence with d.carrier == c. Appending each encountered declaration preserves order and duplicates. Other-carrier declarations previously short-circuited at the first guard without changing the contract. Matching declarations still use the original full catalog, current observations/premises, authority guards, denotation checking and certificate checking. The carrier and catalog cannot change while borrowed. No replay state, predicate result or certificate is memoized.

Falsification: compare the measured source-contract interval and the whole unchanged Dungeon compile using fresh private caches, exact producer/accepted-lock identities, image hashes and runtime oracles. Reject an unsupported gain; unit checks of bucket order alone do not establish performance or language acceptance.


Second candidate: materialize a dedicated carrier/membership-roster result for the lowering source-transfer reader. Source inspection shows that reader consumes only carrier, qualifications and projected qualifications. The existing complete-contract API also materializes local scalar observations and expands the state's shared scalar-premise roster. The new output selection occurs only after the unchanged fixed point has been cached; source-floor checks are unchanged. All existing complete-contract callers request CompleteContract, while the new API returns a distinct result type with no scalar-theory fields. No solver state or result is reused across versions.
