#!/bin/bash
# macOS-specific leak detection using the built-in 'leaks' command
#
# This script runs example binaries under macOS's leak detection tools.
# Use --validate to test that leak detection actually works (runs intentional_leak)

set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "=== macOS Memory Leak Detection ==="
echo

# Check if leaks command is available
if ! command -v leaks &> /dev/null; then
    echo "❌ Error: 'leaks' command not found"
    echo "The 'leaks' command should be available on macOS by default."
    echo "You may need to install Xcode Command Line Tools:"
    echo "  xcode-select --install"
    exit 1
fi

echo "✓ leaks command available"
echo

# Parse command line arguments
VALIDATE_MODE=false
if [ "$1" = "--validate" ]; then
    VALIDATE_MODE=true
    echo "🔍 VALIDATION MODE: Testing leak detection with intentional_leak example"
    echo
fi

# Build examples
echo "Building example binaries in release mode..."
if [ "$VALIDATE_MODE" = true ]; then
    cargo build --release --example intentional_leak --quiet
    echo "✓ Built intentional_leak"
else
    cargo build --release --example leak_check --quiet
    echo "✓ Built leak_check"
fi
echo

# Function to run leak detection
run_leak_check() {
    local example_name=$1
    local binary_path="$PROJECT_ROOT/target/release/examples/$example_name"
    
    if [ ! -f "$binary_path" ]; then
        echo "❌ Error: Binary not found: $binary_path"
        return 1
    fi
    
    echo "Running: $example_name"
    echo "Command: leaks --atExit -- $binary_path"
    echo "---"
    
    # Run with leaks command
    # The leaks command will output leak information if any are found
    leaks --atExit -- "$binary_path" 2>&1 | tee /tmp/leaks_output_$$.txt
    local exit_code=${PIPESTATUS[0]}
    
    echo "---"
    
    # Parse the leak count from output
    # Format: "Process 12345: X leaks for Y total leaked bytes."
    local leak_count=$(grep -o "Process [0-9]*: [0-9]* leaks for" /tmp/leaks_output_$$.txt | grep -o "[0-9]* leaks" | grep -o "^[0-9]*" || echo "0")
    
    if [ "$leak_count" -gt 0 ]; then
        echo "⚠️  LEAKS DETECTED in $example_name: $leak_count leak(s)"
        rm -f /tmp/leaks_output_$$.txt
        return 1
    else
        echo "✓ No leaks detected in $example_name"
        rm -f /tmp/leaks_output_$$.txt
        return 0
    fi
}

# Run leak detection
if [ "$VALIDATE_MODE" = true ]; then
    echo "=== Testing Leak Detection Methodology ==="
    echo
    # For validation, we WANT to detect leaks (return code 1 means leaks found)
    if run_leak_check "intentional_leak"; then
        # No leaks detected - BAD for validation
        echo
        echo "❌ VALIDATION FAILED!"
        echo "The intentional_leak example contains obvious C malloc leaks,"
        echo "but no leaks were detected. The leak detection tool is not working properly."
        exit 1
    else
        # Leaks detected - GOOD for validation
        echo
        echo "✅ VALIDATION PASSED!"
        echo "Leak detection correctly identified the intentional C malloc leaks."
        echo "This proves that the leak detection methodology is working."
        exit 0
    fi
else
    echo "=== Testing Example Binaries ==="
    echo
    
    echo "Running: leak_check (10,000 iterations)"
    if ! run_leak_check "leak_check"; then
        echo
        echo "=== RESULTS ==="
        echo "⚠️  Memory leaks detected!"
        echo
        echo "Review the leak report above for details."
        echo
        echo "Next steps:"
        echo "  1. Review the leak report"
        echo "  2. Check if leaks are in library code or example code"
        echo "  3. Fix memory management issues"
        echo
        echo "To verify leak detection is working, run:"
        echo "  $0 --validate"
        exit 1
    fi
    echo
    
    echo "=== RESULTS ==="
    echo "✅ No memory leaks detected!"
    echo
    echo "The example completed 10,000 iterations without leaking memory."
    echo
    echo "To verify leak detection is working, run:"
    echo "  $0 --validate"
    exit 0
fi

