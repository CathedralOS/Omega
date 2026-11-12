# Distinct nested call footprints

Two invocations of the same helper in one statement write different fields.
Their occurrence ordinals keep the footprints distinct; both writes execute,
and neither retires the guard on the untouched sibling. Native exit is 0.
