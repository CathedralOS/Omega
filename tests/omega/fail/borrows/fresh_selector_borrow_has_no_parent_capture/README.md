A fresh borrow reevaluates mutable `position` after a prior loan captured it.
Both captures select byte zero. The fresh loan has no retained parent-loan
lineage, so it cannot suspend the held loan using inherited-capture authority.
Accepting both would permit simultaneous exclusive writes to the same byte.
