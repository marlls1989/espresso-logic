#!/bin/bash
#
# Regression Test Suite for Espresso Rust CLI
#
# This script runs both the original C CLI and the Rust CLI on test files
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
TEST_DIR="./examples"
TEMP_DIR="./tests/regression_temp"

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Ensure binaries exist
if [ ! -f "$C_BINARY" ]; then
    echo -e "${RED}Error: C binary not found at $C_BINARY${NC}"
    echo "Please run: cd espresso-src && make"
    exit 1
fi

if [ ! -f "$RUST_BINARY" ]; then
    echo -e "${RED}Error: Rust binary not found at $RUST_BINARY${NC}"
    echo "Please run: cargo build --release --bin espresso"
    exit 1
fi

# Create temp directory
mkdir -p "$TEMP_DIR"

echo "╔════════════════════════════════════════════════════════════════════════╗"
echo "║          Espresso Regression Test Suite                                ║"
echo "╚════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "C Binary:    $C_BINARY"
echo "Rust Binary: $RUST_BINARY"
echo "Test Files:  $TEST_DIR"
echo ""

# Test a single file with given options
run_test() {
    local test_file="$1"
    local test_name="$2"
    local options="$3"
    
    local c_output="$TEMP_DIR/c_${test_name}.out"
    local rust_output="$TEMP_DIR/rust_${test_name}.out"
    
    # Run C version
    $C_BINARY $options "$test_file" > "$c_output" 2>/dev/null || {
        echo -e "${YELLOW}SKIP${NC}: $test_name (C binary failed)"
        # Don't count tests where C binary fails
        return
    }
    
    # Only count tests where C binary succeeded
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    # Run Rust version
    $RUST_BINARY $options "$test_file" > "$rust_output" 2>/dev/null || {
        echo -e "${RED}FAIL${NC}: $test_name (Rust binary crashed)"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return
    }
    
    # Compare outputs
    if diff -q "$c_output" "$rust_output" > /dev/null 2>&1; then
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

# Test basic files
echo "Testing basic minimization..."
echo "─────────────────────────────────────────────────────────────────────"

# Find test files
test_files=(
    "examples/ex4"
    "examples/ex5"
    "examples/ex7"
    "examples/b2"
    "examples/b3"
    "examples/b4"
    "examples/b7"
    "examples/in0"
    "examples/in1"
    "examples/in2"
    "examples/m1"
    "examples/m2"
    "examples/t1"
    "examples/t2"
)

for file in "${test_files[@]}"; do
    if [ -f "$file" ]; then
        basename=$(basename "$file")
        run_test "$file" "${basename}" ""
        run_test "$file" "${basename}_fd" "-o fd"
        run_test "$file" "${basename}_fr" "-o fr"
    fi
done

# Test .pla files if they exist
if [ -d "tlex" ]; then
    echo ""
    echo "Testing .pla files..."
    echo "─────────────────────────────────────────────────────────────────────"
    for file in tlex/*.pla; do
        if [ -f "$file" ]; then
            basename=$(basename "$file" .pla)
            run_test "$file" "pla_${basename}" ""
        fi
    done
fi

echo ""
echo "╔════════════════════════════════════════════════════════════════════════╗"
echo "║                         Test Results                                    ║"
echo "╚════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "Total:  $TOTAL_TESTS"
echo -e "Passed: ${GREEN}$PASSED_TESTS${NC}"
echo -e "Failed: ${RED}$FAILED_TESTS${NC}"
echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    echo ""
    echo "The Rust CLI produces identical output to the C CLI."
    rm -rf "$TEMP_DIR"
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    echo ""
    echo "Output files saved in: $TEMP_DIR"
    echo "Investigate differences and update Rust CLI to match C behavior."
    exit 1
fi

