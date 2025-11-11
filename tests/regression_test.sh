#!/bin/bash
#
# Comprehensive Regression Test Suite for Espresso Rust CLI
#
# This script runs both the original C CLI and the Rust CLI on all test files
# and compares their outputs. The C output is considered the reference.
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
TEMP_DIR="./tests/regression_temp"

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Always build C binary to ensure it's up to date
echo -e "${YELLOW}Building C binary...${NC}"
(cd espresso-src && make clean && make) || {
	echo -e "${RED}Failed to build C binary${NC}"
	exit 1
}
echo ""

# Always build Rust binary to ensure it's up to date
echo -e "${YELLOW}Building Rust binary...${NC}"
cargo build --release --bin espresso --features=cli || {
	echo -e "${RED}Failed to build Rust binary${NC}"
	exit 1
}
echo ""

# Create temp directory
mkdir -p "$TEMP_DIR"

echo "╔════════════════════════════════════════════════════════════════════════╗"
echo "║          Comprehensive Espresso Regression Test Suite                  ║"
echo "╚════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "C Binary:    $C_BINARY"
echo "Rust Binary: $RUST_BINARY"
echo ""

# Test a single file with given options
run_test() {
	local test_file="$1"
	local test_name="$2"
	local options="$3"

	local c_output="$TEMP_DIR/c_${test_name}.out"
	local rust_output="$TEMP_DIR/rust_${test_name}.out"

	# Run C version (with timeout to prevent hanging)
	timeout 30 $C_BINARY $options "$test_file" >"$c_output" 2>/dev/null || {
		echo -e "${YELLOW}SKIP${NC}: $test_name (C binary failed or timed out)"
		SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
		return
	}

	# Only count tests where C binary succeeded
	TOTAL_TESTS=$((TOTAL_TESTS + 1))

	# Run Rust version (with timeout to prevent hanging)
	timeout 30 $RUST_BINARY $options "$test_file" >"$rust_output" 2>/dev/null || {
		echo -e "${RED}FAIL${NC}: $test_name (Rust binary crashed or timed out)"
		FAILED_TESTS=$((FAILED_TESTS + 1))
		return
	}

	# Compare outputs
	if diff -q "$c_output" "$rust_output" >/dev/null 2>&1; then
		echo -e "${GREEN}PASS${NC}: $test_name"
		PASSED_TESTS=$((PASSED_TESTS + 1))
	else
		echo -e "${RED}FAIL${NC}: $test_name (outputs differ)"
		FAILED_TESTS=$((FAILED_TESTS + 1))

		# Show differences
		echo "  Diff:"
		diff -u "$c_output" "$rust_output" | head -20 | sed 's/^/    /'
	fi
}

# Test all basic examples with default output
echo "Testing basic minimization (default output)..."
echo "─────────────────────────────────────────────────────────────────────"

for file in pla/ex{4,5,7} pla/b{2,3,4,7,9,10,11,12} pla/{in0,in1,in2,in3,in4,in5,in6,in7} pla/{m1,m2,m3,m4} pla/{t1,t2,t3,t4}; do
	if [ -f "$file" ]; then
		basename=$(basename "$file")
		run_test "$file" "${basename}" ""
	fi
done

# Test all output format variations (f, fd, fr, fdr)
echo ""
echo "Testing output formats (f, fd, fr, fdr)..."
echo "─────────────────────────────────────────────────────────────────────"

for file in pla/ex{4,5,7} pla/b{2,3,4,7} pla/{in0,in1,in2} pla/{m1,m2} pla/{t1,t2}; do
	if [ -f "$file" ]; then
		basename=$(basename "$file")
		run_test "$file" "${basename}_f" "-o f"
		run_test "$file" "${basename}_fd" "-o fd"
		run_test "$file" "${basename}_fr" "-o fr"
		run_test "$file" "${basename}_fdr" "-o fdr"
	fi
done

# Test all .pla files with default output
if [ -d "tlex" ]; then
	echo ""
	echo "Testing all .pla files (default output)..."
	echo "─────────────────────────────────────────────────────────────────────"
	for file in tlex/*.pla; do
		if [ -f "$file" ]; then
			basename=$(basename "$file" .pla)
			run_test "$file" "pla_${basename}" ""
		fi
	done
fi

# Test all .pla files with all output formats
if [ -d "tlex" ]; then
	echo ""
	echo "Testing all .pla files with output formats (f, fd, fr, fdr)..."
	echo "─────────────────────────────────────────────────────────────────────"
	for file in tlex/*.pla; do
		if [ -f "$file" ]; then
			basename=$(basename "$file" .pla)
			run_test "$file" "pla_${basename}_f" "-o f"
			run_test "$file" "pla_${basename}_fd" "-o fd"
			run_test "$file" "pla_${basename}_fr" "-o fr"
			run_test "$file" "pla_${basename}_fdr" "-o fdr"
		fi
	done
fi

echo ""
echo "╔════════════════════════════════════════════════════════════════════════╗"
echo "║                         Test Results                                    ║"
echo "╚════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "Total:   $TOTAL_TESTS"
echo -e "Passed:  ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed:  ${RED}$FAILED_TESTS${NC}"
echo -e "Skipped: ${YELLOW}$SKIPPED_TESTS${NC}"
echo ""

if [ $FAILED_TESTS -eq 0 ] && [ $SKIPPED_TESTS -eq 0 ]; then
	echo -e "${GREEN}✓ All tests passed!${NC}"
	echo ""
	echo "The Rust CLI produces identical output to the C CLI."
	rm -rf "$TEMP_DIR"
	exit 0
elif [ $SKIPPED_TESTS -gt 0 ] && [ $FAILED_TESTS -eq 0 ]; then
	echo -e "${YELLOW}⚠ Some tests were skipped${NC}"
	echo ""
	echo "Skipped tests indicate issues with the C binary (crashes or timeouts)."
	echo "The Rust implementation is working correctly for all tested cases."
	echo ""
	echo "Output files saved in: $TEMP_DIR"
	exit 1
else
	echo -e "${RED}✗ Some tests failed${NC}"
	echo ""
	echo "Output files saved in: $TEMP_DIR"
	echo "Investigate differences and update Rust CLI to match C behavior."
	exit 1
fi
