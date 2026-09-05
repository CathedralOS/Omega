#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon source closure: skipped (python3 absent)"
    exit 0
}

TOOL="$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/materialize_source_closure.py"
FIXTURE_DIR="$TEST_DIR/epsilon-source-closure"
TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM

python3 "$TOOL" "$FIXTURE_DIR/fixture.sources" "$TMP/fixture.epsilon"
[ "$(wc -c < "$TMP/fixture.epsilon" | tr -d ' ')" -eq 89 ]
[ "$(shasum -a 256 "$TMP/fixture.epsilon" | awk '{print $1}')" = \
    "528f65b2e2d9666db1c1f3930c9f5784bbfc1497e3b7225b26cb3eee34d2924c" ]

python3 "$TOOL" "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$TMP/omega_compiler.epsilon"
[ "$(wc -l < "$TMP/omega_compiler.epsilon" | tr -d ' ')" -eq 13572 ]
[ "$(wc -c < "$TMP/omega_compiler.epsilon" | tr -d ' ')" -eq 464741 ]
[ "$(shasum -a 256 "$TMP/omega_compiler.epsilon" | awk '{print $1}')" = \
    "621f507b214f0f26ba3c9d4d36a1bb54a26bdeecbcdffcc24a2cb1a266ab8cde" ]

cp "$FIXTURE_DIR/first.epsilon" "$FIXTURE_DIR/second.epsilon" "$TMP/"
cat > "$TMP/reversed.sources" <<'EOF'
EpsilonSourceClosureV1
member 0000000000000000000000000000000000000000000000000000000000000002 56 9d09f82ca3b097f6ee4cca666dc9da226eea2f9315c23e9c56bec36d7b084e07 second.epsilon
member 0000000000000000000000000000000000000000000000000000000000000001 33 0eb6896c4dade88397a2e690d7bba6463fbc70bb11356c4ece2798107c838e7c first.epsilon
EOF
if python3 "$TOOL" "$TMP/reversed.sources" "$TMP/reversed.epsilon" 2>/dev/null; then
    echo "Epsilon source closure: reversed identities were accepted" >&2
    exit 1
fi

cat > "$TMP/stale.sources" <<'EOF'
EpsilonSourceClosureV1
member 0000000000000000000000000000000000000000000000000000000000000001 33 0000000000000000000000000000000000000000000000000000000000000000 first.epsilon
EOF
if python3 "$TOOL" "$TMP/stale.sources" "$TMP/stale.epsilon" 2>/dev/null; then
    echo "Epsilon source closure: stale member identity was accepted" >&2
    exit 1
fi

cat > "$TMP/escape.sources" <<'EOF'
EpsilonSourceClosureV1
member 0000000000000000000000000000000000000000000000000000000000000001 0 0000000000000000000000000000000000000000000000000000000000000000 ../outside.epsilon
EOF
if python3 "$TOOL" "$TMP/escape.sources" "$TMP/escape.epsilon" 2>/dev/null; then
    echo "Epsilon source closure: escaping member path was accepted" >&2
    exit 1
fi

ln -s "$FIXTURE_DIR" "$TMP/linked"
cat > "$TMP/symlink.sources" <<'EOF'
EpsilonSourceClosureV1
member 0000000000000000000000000000000000000000000000000000000000000001 33 0eb6896c4dade88397a2e690d7bba6463fbc70bb11356c4ece2798107c838e7c linked/first.epsilon
EOF
if python3 "$TOOL" "$TMP/symlink.sources" "$TMP/symlink.epsilon" 2>/dev/null; then
    echo "Epsilon source closure: symbolic-link member path was accepted" >&2
    exit 1
fi

printf 'data Bad {\001}\n' > "$TMP/bad.epsilon"
cat > "$TMP/forbidden.sources" <<'EOF'
EpsilonSourceClosureV1
member 0000000000000000000000000000000000000000000000000000000000000001 13 0000000000000000000000000000000000000000000000000000000000000000 bad.epsilon
EOF
if python3 "$TOOL" "$TMP/forbidden.sources" "$TMP/forbidden.epsilon" 2>/dev/null; then
    echo "Epsilon source closure: forbidden source byte was accepted" >&2
    exit 1
fi

echo "Epsilon source closure: ordered multi-member materialization passes"