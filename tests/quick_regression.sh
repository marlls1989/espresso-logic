#!/bin/bash
#
# Quick Regression Test - Tests core functionality
#

C_BINARY="./bin/espresso"
RUST_BINARY="./target/release/espresso"

# Always build C binary to ensure it's up to date
echo "Building C binary..."
(cd espresso-src && make clean && make) || { echo "Failed to build C binary"; exit 1; }

# Always build Rust binary to ensure it's up to date
echo "Building Rust binary..."
cargo build --release --bin espresso || { echo "Failed to build Rust binary"; exit 1; }
echo ""

echo "Quick Regression Test"
echo "═══════════════════════════════════════"

test_count=0
pass_count=0

run_quick_test() {
    local file="$1"
    local name=$(basename "$file")
    test_count=$((test_count + 1))
    
    if diff <($C_BINARY "$file" 2>/dev/null) <($RUST_BINARY "$file" 2>/dev/null) > /dev/null 2>&1; then
        echo "✓ PASS: $name"
        pass_count=$((pass_count + 1))
    else
        echo "✗ FAIL: $name"
    fi
}

# Test key examples
for file in examples/ex5 examples/ex7 examples/b2 examples/b3; do
    if [ -f "$file" ]; then
        run_quick_test "$file"
    fi
done

echo "═══════════════════════════════════════"
echo "Results: $pass_count/$test_count passed"

if [ $pass_count -eq $test_count ]; then
    echo "✓ All tests passed!"
    exit 0
else
    echo "✗ Some tests failed"
    exit 1
fi

