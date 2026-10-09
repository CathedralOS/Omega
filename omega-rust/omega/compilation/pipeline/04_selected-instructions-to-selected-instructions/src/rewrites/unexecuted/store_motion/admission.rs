//! Shared admission for store mutation motion: locate the named store,
//! prove its single exact write row and operand surface, then walk forward
//! to the latest position that keeps the write ordered before every access
//! that could observe it.
//!
//! The moved store writes the place's storage by one of the routes the other
//! memory rewrites admit: the exact-width `Store` through the referent
//! pointer carrying `WritePlace`, the `StorePacked` an odd fragment width
//! selects through the same pointer routes, or a write into the place's own
//! local storage carrying `WriteLocal` — a `Store` or `StorePacked` through
//! the slot's materialized address, or the always-eight-byte `Store64` into
//! that slot directly — or the byte-sequence store: a `Store { 0, 1 }`
//! through a fully computed view address carrying one `WriteByteSequence`
//! row, whose written byte sits at the row's `byte_offset + index` for the
//! runtime `index` — unless that `index` resolves to a clean
//! `MaterializeI64`, when `admit` collapses the moved extent to the one
//! byte `byte_offset + index` before the walk. The place's own storage is
//! its
//! `StructuralParameter`/`StructuralBlockParameter` slot or the producing
//! operation's `Structural` home — the place's declaration names that
//! producer. A `Structural` slot the declaration does not charge to that
//! operation only stages bytes that name the place (a call's staged view
//! descriptor) under its own slot coordinates, so its write is not a place
//! write: it is the moved store of the slot's own bytes instead. Staged
//! bytes sit under slot coordinates no place-named row reaches — reads of
//! them would have no honest roster role, and the consuming operation sits
//! behind the call barrier — so only a row naming that very slot can observe
//! or rewrite them, and the staging store sinks under the same walk bounded
//! by the first access on the slot's own range.
//!
//! The packed form's early-clobber scratch `Def` and declared clobbers move
//! with the instruction, so custody is proven as the write side of the same
//! register/condition-state coupling the walk already enforces for the
//! store's reads: any use of the scratch inside the window would read the
//! moved definition early, any second write would be overtaken by it, and a
//! flag read or write would cross the moved flag clobber — each bounds the
//! motion at the position that mentions it.
//!
//! Sinking the store delays the write inside the window where the delay is
//! unobservable. The scan stops before the first position that must stay
//! ordered after the store: a roster row touching the moved byte range — an
//! exact row intersecting it, or a dynamic-extent row whose fixed offset
//! still starts below the moved end, its reach being unbounded upward —
//! while a byte-sequence row whose `index` resolves to a clean
//! `MaterializeI64` touches exactly the byte `byte_offset + index`, so it
//! stops the slide only by landing inside the moved extent and a landing
//! anywhere off it walks past. Then a
//! call or hosted effect, a redefinition of a
//! carried register, a memory-capable instruction with no roster row, or a
//! boundary settlement. The store lands immediately before that position;
//! every kept event still observes the write in the same relative order.
//! When the moved store is itself a byte-sequence write the reach direction
//! mirrors: the written byte can sit anywhere at or past the row's fixed
//! offset, so a later exact or local row still reaches it once its own
//! extent ends past that offset, and a later dynamic-extent row on the
//! place always meets it — unless the moved `index` resolved the same way,
//! collapsing the extent to that one byte before the walk, so a resolved
//! row's landing decides and an unresolved row still reaches it only from
//! a payload base at or below it.
//!
//! The walk is not confined to one block: reaching a block's end without a
//! stop continues through its terminator's successor edges when every edge
//! names one block, that block's only predecessor is the crossed block, and
//! the successor cannot reach any block already walked. A fork crosses too
//! when its edges reconverge — each lands on one join directly or enters a
//! region fed only through the fork's own edges: a block joins the region
//! once every predecessor leads from the crossed block or the region
//! itself, so entry happens at the fork and the region stays acyclic, every
//! region block carries at least one successor, and every edge leaving the
//! region lands on that join — a chain or a nested reconvergent shape
//! inside one arm crosses under the same count. The join's only
//! predecessor blocks are the crossed block and the region, and every
//! region block stays walkable end to end — so every traversal of the
//! crossed block reaches the join exactly once and the write runs once
//! there as it ran once here: no path gains or drops it. A fork whose
//! targets never reconverge would drop the write from the paths that leave
//! it; a join or a region block fed from outside the region would add the
//! write to paths that never carried it; and a successor that can return
//! to the walked chain
//! would close a cycle in
//! which an access before the store's original position now runs after it —
//! the walked interval only proves the forward half of that order. Each
//! crossed terminator's roster rows and register definitions decide before
//! the edge's transports, which may not redefine the carried registers or
//! write the moved place's storage. Every failed crossing lands the store at
//! the crossed block's end instead of rejecting: an earlier landing on the
//! proven path is still a real motion.
