# Shared environment preamble for the owner-local run.sh gates. The
# caller sets GATE_DIR to its own directory and GATE_ENVS to the
# space-separated rung environment files it needs, relative to
# tools/bootstrap (e.g. GATE_ENVS="delta/compiler_env.sh"); sourcing this file
# resolves the repository root, exposes paths.sh, and sources each rung env.
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    [ "$OMEGA_REPO_ROOT" != / ] || { echo "cannot locate tools/bootstrap/paths.sh" >&2; return 1; }
    OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
for _gate_env in $GATE_ENVS; do
    . "$OMEGA_REPO_ROOT/tools/bootstrap/$_gate_env"
done

# Install a `python3` PATH shim that rewrites every path-shaped argument to
# Windows form through cygpath, so a Windows Python on the MINGW/MSYS route
# consumes the caller's MSYS paths. Callers gate on `command -v cygpath`;
# POSIX hosts keep native paths untouched.
install_mingw_python3_shim() {
    _shim_dir=$1
    REAL_PYTHON3=$(command -v python3)
    mkdir -p "$_shim_dir"
    {
        printf '%s\n' '#!/usr/bin/env sh'
        printf '%s\n' 'n=$#'
        printf '%s\n' 'while [ "$n" -gt 0 ]; do'
        printf '%s\n' '    arg=$1; shift; n=$((n - 1))'
        printf '%s\n' '    case $arg in'
        printf '%s\n' '        -* | "") ;;'
        printf '%s\n' '        */* | *\\* | ?:*)'
        printf '%s\n' '            arg=$(cygpath -w "$arg" 2>/dev/null || printf "%s" "$arg") ;;'
        printf '%s\n' '    esac'
        printf '%s\n' '    set -- "$@" "$arg"'
        printf '%s\n' 'done'
        printf 'exec %s "$@"\n' "'$REAL_PYTHON3'"
    } > "$_shim_dir/python3"
    chmod +x "$_shim_dir/python3"
    PATH=$_shim_dir:$PATH
    export PATH
}
