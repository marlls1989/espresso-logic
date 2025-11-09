#!/bin/bash
#
# Quick Regression Test - Tests a few key examples for rapid iteration
#
# This is a fast sanity check during development. For comprehensive testing,
# use regression_test.sh instead.
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Paths
C_BINARY="./bin/espresso"
RUST_BINARY="./target/release/espresso"
TEMP_DIR="./tests/regression_temp_quick"

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Build binaries (no clean to save time)
echo -e "${YELLOW}Building binaries (incremental)...${NC}"
(cd espresso-src && make) >/dev/null 2>&1 || {
	echo -e "${RED}Failed to build C binary${NC}"
	exit 1
}
cargo build --release --bin espresso --features=cli >/dev/null 2>&1 || {
	echo -e "${RED}Failed to build Rust binary${NC}"
	exit 1
}

# Create temp directory
mkdir -p "$TEMP_DIR"

echo "╔════════════════════════════════════════════════════════════════════════╗"
echo "║          Quick Regression Test (4 tests)                               ║"
echo "╚════════════════════════════════════════════════════════════════════════╝"
echo ""

run_quick_test() {
	local file="$1"
	local name=$(basename "$file")
	
	local c_output="$TEMP_DIR/c_${name}.out"
	local rust_output="$TEMP_DIR/rust_${name}.out"

	# Run C version (with timeout to prevent hanging)
	timeout 10 $C_BINARY "$file" >"$c_output" 2>/dev/null || {
		echo -e "${YELLOW}SKIP${NC}: $name"
		SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
		return
	}

	# Only count tests where C binary succeeded
	TOTAL_TESTS=$((TOTAL_TESTS + 1))

	# Run Rust version (with timeout to prevent hanging)
	timeout 10 $RUST_BINARY "$file" >"$rust_output" 2>/dev/null || {
		echo -e "${RED}FAIL${NC}: $name"
		FAILED_TESTS=$((FAILED_TESTS + 1))
		return
	}

	# Compare outputs
	if diff -q "$c_output" "$rust_output" >/dev/null 2>&1; then
		echo -e "${GREEN}PASS${NC}: $name"
		PASSED_TESTS=$((PASSED_TESTS + 1))
	else
		echo -e "${RED}FAIL${NC}: $name"
		FAILED_TESTS=$((FAILED_TESTS + 1))
	fi
}

# Test a few key examples for quick feedback
for file in examples/ex5 examples/b3 examples/in1 examples/t1; do
	if [ -f "$file" ]; then
		run_quick_test "$file"
	fi
done

echo ""
echo "Results: $PASSED_TESTS/$TOTAL_TESTS passed"
if [ $FAILED_TESTS -eq 0 ] && [ $SKIPPED_TESTS -eq 0 ]; then
	echo -e "${GREEN}✓ Quick check passed!${NC}"
	rm -rf "$TEMP_DIR"
	exit 0
else
	echo -e "${RED}✗ Issues detected - run ./tests/regression_test.sh for details${NC}"
	exit 1
fi

