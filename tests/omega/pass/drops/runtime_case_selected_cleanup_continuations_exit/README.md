# Runtime-selected cleanup continuations

Normal return, projected return and an ordinary state transfer dispose only the
active sum payload. The parent hook retains child custody across the private
selection blocks, and mutating a marker during cleanup cannot change the scalar
result already captured before the hook. Projected outgoing values retain their
authored contents after the old owner's remaining regions and home are disposed.
Borrowed call-result temporaries use the same selected cleanup after their scalar
consumer completes, including an empty result with no fabricated payload debt.

Expected native and Terminal exit: 0.
