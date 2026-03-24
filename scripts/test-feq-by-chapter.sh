#!/bin/bash
# Test veracity-full-generic-feq chapter-by-chapter against the fixture.
#
# For each chapter: apply the tool, validate, record timing.
# This identifies which chapter (if any) causes a proof time spiral.
#
# Usage: scripts/test-feq-by-chapter.sh

set -uo pipefail

VERACITY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="$VERACITY_ROOT/tests/fixtures/APAS-VERUS"
TOOL="$VERACITY_ROOT/target/release/veracity-full-generic-feq"
VERUS=~/projects/verus/source/target-verus/release/verus
LOGDIR="$VERACITY_ROOT/logs/feq-by-chapter"
TIMEOUT_SEC=300

CHAPTERS=(Chap05 Chap37 Chap41 Chap42 Chap43 Chap45 Chap65)

mkdir -p "$LOGDIR"
SUMMARY="$LOGDIR/summary.txt"
> "$SUMMARY"

echo "=== feq-by-chapter test ==="
echo "Fixture: $FIXTURE"
echo "Chapters: ${CHAPTERS[*]}"
echo ""

# Step 0: Fresh fixture
echo "--- Cloning fresh fixture ---"
cd "$VERACITY_ROOT/tests/fixtures"
rm -rf APAS-VERUS
git clone https://github.com/briangmilnes/APAS-VERUS.git APAS-VERUS 2>&1 | tail -1
echo ""

# Step 1: Baseline validation (no changes)
echo "--- Baseline validation ---"
cd "$FIXTURE"
START=$(date +%s)
timeout "$TIMEOUT_SEC" "$VERUS" --crate-type=lib src/lib.rs --multiple-errors 20 --expand-errors \
    --num-threads 8 --time 2>&1 | sed 's/\x1b\[[0-9;]*m//g' > "$LOGDIR/00-baseline.log"
RC=$?
ELAPSED=$(( $(date +%s) - START ))

ERRORS=$(grep -c '^error:' "$LOGDIR/00-baseline.log" 2>/dev/null || echo 0)
SMT=$(grep 'total smt-time:' "$LOGDIR/00-baseline.log" | awk '{print $3}' || echo "?")
TOTAL=$(grep '^total-time:' "$LOGDIR/00-baseline.log" | awk '{print $2}' || echo "?")

printf "%-12s  rc=%-3s  errors=%-3s  total=%s ms  smt=%s ms  wall=%ss\n" \
    "baseline" "$RC" "$ERRORS" "$TOTAL" "$SMT" "$ELAPSED" | tee -a "$SUMMARY"
echo ""

# Step 2: Apply tool chapter-by-chapter
for CHAP in "${CHAPTERS[@]}"; do
    echo "--- Applying $CHAP ---"
    cd "$VERACITY_ROOT"

    # Apply the tool (not dry-run)
    "$TOOL" -c "$FIXTURE" -d "src/$CHAP" 2>&1 | tee "$LOGDIR/$CHAP-apply.log"
    echo ""

    # Validate
    echo "--- Validating after $CHAP ---"
    cd "$FIXTURE"
    START=$(date +%s)
    timeout "$TIMEOUT_SEC" "$VERUS" --crate-type=lib src/lib.rs --multiple-errors 20 --expand-errors \
        --num-threads 8 --time 2>&1 | sed 's/\x1b\[[0-9;]*m//g' > "$LOGDIR/$CHAP-validate.log"
    RC=$?
    ELAPSED=$(( $(date +%s) - START ))

    ERRORS=$(grep -c '^error:' "$LOGDIR/$CHAP-validate.log" 2>/dev/null || echo 0)
    SMT=$(grep 'total smt-time:' "$LOGDIR/$CHAP-validate.log" | awk '{print $3}' || echo "?")
    TOTAL=$(grep '^total-time:' "$LOGDIR/$CHAP-validate.log" | awk '{print $2}' || echo "?")

    printf "%-12s  rc=%-3s  errors=%-3s  total=%s ms  smt=%s ms  wall=%ss\n" \
        "$CHAP" "$RC" "$ERRORS" "$TOTAL" "$SMT" "$ELAPSED" | tee -a "$SUMMARY"
    echo ""
done

echo "=== Summary ==="
cat "$SUMMARY"
echo ""
echo "Full logs in: $LOGDIR"
