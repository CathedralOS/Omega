The checked provider proves a disjoint `IntervalSet<ByteUnit>` split and returns its pieces in reverse coordinate order. Its caller disposes one piece, returns the other, checks both endpoints, then disposes the remainder. Three calls exercise an interior partition and both empty-piece boundaries. Expected native and Terminal exit: 70.

Both pieces must be well ordered and disjoint before endpoint coverage can establish conservation. Empty pieces are permitted at either partition boundary.
