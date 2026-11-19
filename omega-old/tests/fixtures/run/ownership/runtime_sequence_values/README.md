The two returned collections carry different runtime field values inside records.
Both remain live before by-value calls read their elements; an empty collection
also keeps its own live length and takes the public reader’s refusal branch. This exercises aggregate returns with inline backing.

The negative changes only the public reader’s guard from `len == 2` to `len >= 1`.
Its explicit `requires true` keeps checking independent of its current callers.
The second element read must refuse before execution.
