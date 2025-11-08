#!/bin/bash
# Cross-platform memory leak detection script
#
# This script delegates to platform-specific leak detection:
# - macOS: leaks command on example binaries
# - Linux: valgrind or heaptrack on example binaries

set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "=== Memory Leak Detection for Espresso Logic ==="
echo

# Parse command line arguments
ARGS="$@"

# Detect platform and delegate
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS - delegate to macOS-specific script
    if [ -f "$PROJECT_ROOT/scripts/check_leaks_macos.sh" ]; then
        echo "Platform: macOS"
        echo "Method: leaks command"
        echo "Delegating to check_leaks_macos.sh..."
        echo
        exec "$PROJECT_ROOT/scripts/check_leaks_macos.sh" $ARGS
    else
        echo "❌ Error: check_leaks_macos.sh not found"
        exit 1
    fi
else
    # Linux - use valgrind or heaptrack
    echo "Platform: Linux"
    echo
    
    # Check which tools are available
    HAS_VALGRIND=false
    HAS_HEAPTRACK=false
    
    if command -v valgrind &> /dev/null; then
        HAS_VALGRIND=true
        echo "✓ valgrind available"
    fi
    
    if command -v heaptrack &> /dev/null; then
        HAS_HEAPTRACK=true
        echo "✓ heaptrack available"
    fi
    
    if [ "$HAS_VALGRIND" = false ] && [ "$HAS_HEAPTRACK" = false ]; then
        echo "❌ No leak detection tool available"
        echo
        echo "To install:"
        echo "  sudo apt install valgrind"
        echo "  sudo apt install heaptrack"
        exit 1
    fi
    
    echo
    
    # Build example binaries
    echo "Building example binaries..."
    cargo build --release --example leak_check --quiet
    echo "✓ Built leak_check"
    echo
    
    # Function to run leak check with valgrind
    run_valgrind() {
        local example_name=$1
        local binary_path="$PROJECT_ROOT/target/release/examples/$example_name"
        
        echo "=== Running valgrind on $example_name ==="
        valgrind \
            --leak-check=full \
            --show-leak-kinds=all \
            --track-origins=yes \
            --error-exitcode=1 \
            "$binary_path"
        
        if [ $? -eq 0 ]; then
            echo "✓ No leaks detected in $example_name"
            return 0
        else
            echo "⚠️  Leaks or errors detected in $example_name"
            return 1
        fi
    }
    
    # Function to run leak check with heaptrack
    run_heaptrack() {
        local example_name=$1
        local binary_path="$PROJECT_ROOT/target/release/examples/$example_name"
        
        echo "=== Running heaptrack on $example_name ==="
        heaptrack "$binary_path"
        echo "✓ heaptrack profiling complete for $example_name"
        echo "Run 'heaptrack_gui heaptrack.*.gz' to view results"
        return 0
    }
    
    # Run leak detection
    if [ "$HAS_VALGRIND" = true ]; then
        if ! run_valgrind "leak_check"; then
            echo "=== RESULT: LEAKS DETECTED ==="
            echo "❌ Memory leaks were found."
            exit 1
        fi
    elif [ "$HAS_HEAPTRACK" = true ]; then
        run_heaptrack "leak_check"
    fi
    
    echo "=== RESULT: NO LEAKS ==="
    echo "✅ No memory leaks detected."
    exit 0
fi

