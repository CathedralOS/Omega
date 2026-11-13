# Atomic load after distinct branch writes

Two calls exercise both branches: one stores 37 and the other stores 91 before reaching the same atomic load. The caller checks both distinct results, the final resident value, and unchanged neighboring fields 13 and 29. A mismatch traps; the expected exit is 0.

Atomic lowering must retain both incoming write histories and observe the branch actually taken. Choosing either write for every arrival loses the other branch's meaning.

The repaired producer runs both branches with native exit 0 and Terminal Psi exit 0. This is a composition control for the witnessed loop defect, not a separately measured baseline failure. It makes no hardware interrupt claim.

Focused ordering rejection: [a load using Publish](../../../fail/atomics/atomic_load_publish_ordering_rejected/main.omg) must continue to reject. Keeping complete incoming histories does not make an invalid memory ordering legal.
