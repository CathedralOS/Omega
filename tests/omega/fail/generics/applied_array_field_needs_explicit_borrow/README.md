# Named calls require an explicit borrowed argument

An applied fixed-array field passed by value does not gain the indexed
operator shell conversion merely because the static predicate fixes T.
The ordinary named count call still needs an explicit borrow.
