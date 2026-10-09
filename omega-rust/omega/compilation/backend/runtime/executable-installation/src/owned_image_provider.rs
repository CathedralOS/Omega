//! Owned-image provider performing the contracted installation and patching
//! operations over the resident image buffers it holds.
//!
//! The lifecycle transitions in `executable_installation.rs` consume provider
//! evidence; this module is a provider that produces it by doing the work. It
//! owns one resident image per installed realization: `install` copies the
//! validated final bytes into that custody, orders the stores, reads the image
//! back, and suspends its own write authority before reporting the
//! write-to-execute transition; `patch` splices each demanded declared entry
//! site with its bound admitted fragment, re-suspending write authority over
//! the patched image afterward; `seal_entry` performs the entry-sealing work
//! `InstalledCode::seal_entry_reference` contracts for, and `call` is the
//! provider's call path — the only route that turns a sealed
//! `InstalledEntryReference` into the resident entry content a physical call
//! enters; `retire` unwinds the write-to-execute transition for a drained
//! realization, and `quarantine` parks a drain that cannot complete as an
//! unserved trapping reservation — both reporting the executor quiescence
//! the `&mut self` receiver establishes structurally. Receipts are minted
//! only for steps actually performed, and a demand for facts outside this
//! provider's performed set is refused before any mutation.
//!
//! Honest boundaries: an owned buffer cannot be hardware-protected, so this
//! provider reports `WxEnforcement::ConventionOnly` — write authority is the
//! provider's own custody flag, resumed only inside `patch` and re-suspended
//! before the receipt reports it. Cache-order and instruction-fetch claims
//! rest on the performed `SeqCst` fence plus read-back comparison of the
//! stored bytes, never on an unperformed platform cache operation. Patching
//! splices position-independent fragments only: a fragment carrying
//! relocations needs a target resolver this operation does not have, so it
//! refuses rather than patching unresolved bytes. `seal_entry` replays the
//! exact committed content of the demanded entry extent — installed bytes,
//! or the fragment committed at a patched site — so a drifted or substituted
//! entry cannot seal. `call` hands the caller's executor the resident entry
//! bytes under seal, not a runnable address: the physical control transfer
//! remains the consuming platform executor's obligation, and holding the
//! returned `ResidentEntryCall` keeps the image borrowed so no `patch` or
//! `release` can run while a call is in flight — and that same exclusive
//! borrow is the executor-quiescence evidence `retire` and `quarantine`
//! report. Retirement clears the image's execute-enabled flag and restores
//! write authority; quarantine marks the image trapping — every serve path
//! refuses, and `release` refuses while the range stays reserved — so a stale
//! entry attempt faults at the provider rather than being served or
//! silently freed. Which ending a failed drain takes, and the attributed
//! residual holder the quarantine receipt names, stay caller obligations.

