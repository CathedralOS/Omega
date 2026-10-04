The true guard returns the caller's mutable reference before the subsequent crash. Reading the returned reference must observe the original value, and both engines must exit zero.
