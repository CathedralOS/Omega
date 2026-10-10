# Selected index cannot borrow mutable storage from a shared parent

The selected index has a mutable collection formal. Its generated borrow of
the caller's field must still fail ordinary exclusive-access admission.
