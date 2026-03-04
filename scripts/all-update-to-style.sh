#!/bin/bash
# Run veracity-update-to-style -C on each chapter directory.
# The tool writes its own log to $dir/analyses/veracity-update-to-style.log.
#
# Usage:
#   scripts/all-update-to-style.sh                    # dry-run (default)
#   scripts/all-update-to-style.sh --no-dry-run       # live run
#   scripts/all-update-to-style.sh <fixture-path>     # custom fixture root

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERACITY="$SCRIPT_DIR/../target/release/veracity-update-to-style"
DEFAULT_FIXTURE="$SCRIPT_DIR/../tests/fixtures/APAS-VERUS"

DRY_RUN="-n"
FIXTURE="$DEFAULT_FIXTURE"

for arg in "$@"; do
    case "$arg" in
        --no-dry-run)
            DRY_RUN=""
            ;;
        *)
            FIXTURE="$arg"
            ;;
    esac
done

if [ ! -x "$VERACITY" ]; then
    echo "Binary not found: $VERACITY"
    echo "Run: cargo build --release -p veracity --bin veracity-update-to-style"
    exit 1
fi

if [ ! -d "$FIXTURE/src" ]; then
    echo "Fixture src/ not found: $FIXTURE/src"
    exit 1
fi

cd "$FIXTURE"

for dir in src/Chap*/; do
    mkdir -p "$dir/analyses"
    "$VERACITY" -C $DRY_RUN -c "$FIXTURE" "$dir"
    echo
done
