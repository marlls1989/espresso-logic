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

# Build examples UNOPTIMIZED (debug). Release optimisation elides allocations whose result never
# escapes — which silently removes the intentional leaks in intentional_leak (so `leaks` would report
# none) and could likewise mask a real leak in leak_check. Debug keeps every allocation intact.
echo "Building example binaries (debug, so no allocation is optimised away)..."
if [ "$VALIDATE_MODE" = true ]; then
    cargo build --example intentional_leak --quiet
    echo "✓ Built intentional_leak"
else
    cargo build --example leak_check --quiet
    echo "✓ Built leak_check"
fi
echo

# Run leak detection by ATTACHING to a live process (`leaks <pid>`), not `leaks --atExit`.
#
# Why: `leaks --atExit` injects libLeaksAtExit.dylib, which interposes exit() and makes the target
# SIGSTOP itself so the parent `leaks` can scan it at exit. On macOS 26 the target is "not debuggable"
# (security restrictions), so `leaks` scans it but cannot resume the stopped process; it exits and
# leaves the target orphaned in a stopped state, still holding the report pipe's write end — so the
# `| tee` never sees EOF and the whole script hangs forever after printing "0 leaks".
#
# Attaching to a live process avoids that: `leaks <pid>` suspends and RESUMES the target via task
# ports, never leaving it stopped. The example runs its workload, prints "READY <pid>", then parks on
# stdin (driven by ESPRESSO_LEAK_PARK) so we can scan it live and then release it for a clean exit.
run_leak_check() {
    local example_name=$1
    local binary_path="$PROJECT_ROOT/target/debug/examples/$example_name"

    if [ ! -f "$binary_path" ]; then
        echo "❌ Error: Binary not found: $binary_path"
        return 1
    fi

    # Ad-hoc code-sign the binary with the `get-task-allow` entitlement. Without it, macOS 26 marks
    # the process "not debuggable" and `leaks` can only read read-only memory — so it cannot scan the
    # heap and reports zero leaks regardless of what was actually leaked.
    local ent_plist
    ent_plist=$(mktemp)
    cat >"$ent_plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.get-task-allow</key>
    <true/>
</dict>
</plist>
PLIST
    codesign -s - -f --entitlements "$ent_plist" "$binary_path" >/dev/null 2>&1 || {
        echo "❌ Error: failed to code-sign $example_name with get-task-allow"
        rm -f "$ent_plist"
        return 1
    }
    rm -f "$ent_plist"

    echo "Running: $example_name"
    echo "Command: leaks <pid>  (attach mode against the live process)"
    echo "---"

    local out_file report_file ctrl_fifo
    out_file=$(mktemp)
    report_file=$(mktemp)
    ctrl_fifo=$(mktemp -u)
    mkfifo "$ctrl_fifo"

    # Launch the example with stdin connected to a control FIFO. Hold the FIFO open for writing on fd
    # 3 so the example's stdin does not hit EOF until we release it. MallocStackLogging lets `leaks`
    # enumerate every allocation (and report allocation stacks) on macOS 26.
    exec 3<>"$ctrl_fifo"
    MallocStackLogging=1 ESPRESSO_LEAK_PARK=1 "$binary_path" <"$ctrl_fifo" >"$out_file" 2>&1 &
    local pid=$!

    # Wait (bounded) for the example to finish its workload and park, ready to be scanned.
    local waited=0
    while ! grep -q "READY" "$out_file" 2>/dev/null; do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "❌ Error: $example_name exited before it was ready to scan"
            cat "$out_file"
            exec 3>&-
            rm -f "$ctrl_fifo" "$out_file" "$report_file"
            return 1
        fi
        sleep 0.1
        waited=$((waited + 1))
        if [ "$waited" -ge 600 ]; then # 60s ceiling — should be reached in well under a second
            echo "❌ Error: $example_name did not become ready within 60s"
            kill "$pid" 2>/dev/null
            exec 3>&-
            rm -f "$ctrl_fifo" "$out_file" "$report_file"
            return 1
        fi
    done

    # Scan the live process. (Attach mode resumes the target afterwards — no orphaned stopped process.)
    leaks "$pid" 2>&1 | tee "$report_file"

    # Release the example so it exits cleanly on its own, then reap it (no kill needed).
    echo "scan-done" >&3
    exec 3>&-
    wait "$pid" 2>/dev/null || true
    rm -f "$ctrl_fifo"

    # Show the example's own output (workload progress + Done).
    cat "$out_file"
    echo "---"

    # Parse the leak count. Format: "Process 12345: X leaks for Y total leaked bytes."
    local leak_count
    leak_count=$(grep -o "Process [0-9]*: [0-9]* leaks for" "$report_file" | grep -o "[0-9]* leaks" | grep -o "^[0-9]*" || echo "0")
    rm -f "$out_file" "$report_file"

    if [ "$leak_count" -gt 0 ]; then
        echo "⚠️  LEAKS DETECTED in $example_name: $leak_count leak(s)"
        return 1
    else
        echo "✓ No leaks detected in $example_name"
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

