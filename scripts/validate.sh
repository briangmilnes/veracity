#!/bin/bash
# Run fixture validation. Usage: scripts/validate.sh [full|dev_only|exp] [--time]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE="$SCRIPT_DIR/../tests/fixtures/APAS-VERUS"

cd "$FIXTURE"
exec ./scripts/validate.sh "$@"
