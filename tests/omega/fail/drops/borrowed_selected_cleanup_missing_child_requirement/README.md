# Selected parent cleanup cannot erase a child's prerequisite

The parent establishes child value 8, while the child's drop requires 7.
Ordering cleanup inside a runtime-selected receiver must still reject this
missing predicate; accepting the selector does not authorize either hook.
