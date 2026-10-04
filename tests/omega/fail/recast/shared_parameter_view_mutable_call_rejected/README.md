A reference formed from an immutable scalar parameter remains shared when passed
to a helper. Call composition must reject an exclusive formal without converting
the reference carrier into ownership of the borrowed storage.
