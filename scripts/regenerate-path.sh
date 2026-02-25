#!/bin/bash
# Regenerate path/ from src in the APAS-VERUS fixture.
# Usage: scripts/regenerate-path.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE="$SCRIPT_DIR/../tests/fixtures/APAS-VERUS"

cd "$FIXTURE"
exec ./scripts/regenerate-path.sh
