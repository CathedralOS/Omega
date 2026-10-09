//! Independent replay of retained structural-result `Call` reference rosters.
//!
//! A structural-result `Call` carries `reference_results`: the ordered leaf
//! roster declaring which suspended referent root each returned reference
//! carrier names. The producer resolves that roster through live custody —
//! owned arguments move their complete leaf set, `.., Referent` mappings
//! descend through the callee's formal ingress, and surviving leaves keep the
//! root they were formed with. This module recomputes the same roster from the
//! caller's own operation stream alone: entry seeding, carrier moves, record
//! relocations, call results, and edge discards reach a fixed point over actual
//! predecessor exits. Selected owned parameter leaves retain every incoming
//! root; all other live leaves agree exactly. Only dominating definitions, not
//! custody, are inherited from the dominator. Cached entries are replaced from
//! current exits, never merged with stale snapshots from an earlier iteration.
//! Retained rows are compared against this independent derivation rather than
//! trusted. Producer admission gates that cannot change an emitted row —
//! suspended-root checks, parent/child exclusivity, referent typing — are
//! source verification's concern, so this replay only tracks which leaf lives
//! at which carrier path and which root it suspends.


