//! The replacement join over the patch and drain transitions
//! (wiki/spec/build/executable_installation.md#visibility-and-retirement).
//!
//! Replacing a live installed realization routes new calls first and drains
//! the superseded era afterward. Only a realization installed for the
//! possible-current-executor audience takes this path — live-site patching is
//! the alteration the executor audience exists to require, and routing its
//! calls hands them to a successor that must share that audience; a dormant
//! or future-fetcher realization is altered by retiring and reinstalling
//! instead. The provider must patch every site the
//! authority demands with the admitted fragment bound to it — the only code
//! sites an artifact declares are its entries, so no other offset may carry a
//! patch — while write authority is re-suspended and instruction-fetch
//! visibility completes over the patched bytes. Only an established patch
//! reaches the drain: retirement returns the placement for reuse, an
//! incomplete drain with trapping evidence parks in quarantine, and any
//! refusal returns every supplied input, leaving the realization's custody
//! untouched.

