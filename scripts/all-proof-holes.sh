#!/bin/bash
# Run veracity-review-proof-holes on the fixture (all chapters).
# Output: $FIXTURE/analyses/veracity-review-verus-proof-holes.log
#
# Usage:
#   scripts/all-proof-holes.sh                    # default fixture
#   scripts/all-proof-holes.sh <fixture-path>     # custom fixture root

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERACITY="$SCRIPT_DIR/../target/release/veracity-review-proof-holes"
DEFAULT_FIXTURE="$SCRIPT_DIR/../tests/fixtures/APAS-VERUS"

FIXTURE="$DEFAULT_FIXTURE"
for arg in "$@"; do
    FIXTURE="$arg"
done

if [ ! -x "$VERACITY" ]; then
    echo "Binary not found: $VERACITY"
    echo "Run: cargo build --release -p veracity --bin veracity-review-proof-holes"
    exit 1
fi

if [ ! -d "$FIXTURE/src" ]; then
    echo "Fixture src/ not found: $FIXTURE/src"
    exit 1
fi

cd "$FIXTURE"

# -d . uses fixture root as base_dir so log -> $FIXTURE/analyses/
"$VERACITY" -d . -e src/experiments -e src/vstdplus -e benches -e tests -e rust_verify_test
echo
echo "Log: $FIXTURE/analyses/veracity-review-verus-proof-holes.log"
