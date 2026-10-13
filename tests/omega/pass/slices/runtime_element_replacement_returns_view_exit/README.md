Replacing a runtime-selected record element changes the borrowed backing, not
the descriptor used to reach it. Returning that descriptor preserves its exact
input-relative origin. The caller observes the replacement through both the
returned view and the original array, including a wide tag and untouched
sentinels outside the selected window.
