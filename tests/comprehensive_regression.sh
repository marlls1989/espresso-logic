#!/bin/bash
#
# Comprehensive Regression Test
# Tests Rust CLI against C CLI on many test files
#

C_BINARY="./bin/espresso"
RUST_BINARY="./target/release/espresso"

# Always build C binary to ensure it's up to date
echo "Building C binary..."
(cd espresso-src && make clean && make) || { echo "Failed to build C binary"; exit 1; }
echo ""

# Always build Rust binary to ensure it's up to date
echo "Building Rust binary..."
cargo build --release --bin espresso || { echo "Failed to build Rust binary"; exit 1; }
echo ""

test_count=0
pass_count=0
fail_count=0

echo "Comprehensive Regression Test"
echo "═════════════════════════════════════════════════════════════"

run_test() {
    local file="$1"
    local opts="$2"
    local name=$(basename "$file")$([ -n "$opts" ] && echo " $opts" || echo "")
    test_count=$((test_count + 1))
    
    if diff <($C_BINARY $opts "$file" 2>/dev/null) <($RUST_BINARY $opts "$file" 2>/dev/null) > /dev/null 2>&1; then
        echo "✓ $name"
        pass_count=$((pass_count + 1))
    else
        echo "✗ $name"
        fail_count=$((fail_count + 1))
    fi
}

# Test examples directory
echo "Testing examples/..."
for file in examples/ex{4,5,7} examples/b{2,3,4,7,9,10,11,12} examples/{in0,in1,in2,in3,in4,in5,in6,in7} examples/{m1,m2,m3,m4} examples/{t1,t2,t3,t4}; do
    [ -f "$file" ] && run_test "$file" ""
done

# Test with different output formats
echo ""
echo "Testing output formats..."
for file in examples/ex5 examples/b3 examples/m1; do
    [ -f "$file" ] && run_test "$file" "-o fd"
    [ -f "$file" ] && run_test "$file" "-o fr"
done

# Test PLA files
echo ""
echo "Testing tlex/*.pla files..."
for file in tlex/{ex5,mytest,inc,vg2,bw}.pla; do
    [ -f "$file" ] && run_test "$file" ""
done

echo ""
echo "═════════════════════════════════════════════════════════════"
echo "Results: $pass_count/$test_count passed"
[ $fail_count -eq 0 ] && echo "✓ ALL TESTS PASSED!" || echo "✗ $fail_count tests failed"
echo "═════════════════════════════════════════════════════════════"

exit $fail_count

