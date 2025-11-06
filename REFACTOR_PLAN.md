# Espresso-Logic Refactoring Plan

## Current Status (Context Checkpoint)

### What's Working ✅
- All 39 Rust unit tests pass
- Outputs changed to `Option<bool>` to support don't-cares (Some(true)=1, Some(false)=0, None=don't-care)
- F-type PLA handling works correctly: **9/9 F-type regression tests pass** (ex4, ex5, ex7, b2, in1, in2, m1, m2, t1)
- Process isolation works (debug/trace flags redirect to stderr correctly)
- CLI fully integrated with new API
- PLA file parsing with cube merging (lines with same inputs get merged via OR logic)

### What's Broken ❌
- **19/28 regression tests fail** - all are FD-type (with `.type fd` directive)
- FD/FR/FDR type PLAs not handled correctly in cube splitting
- Current implementation only creates F cubes, ignores D and R requirements

### Root Cause Analysis
The C code (cvrin.c lines 176-199) creates up to 3 cubes per PLA line:
- **F cube**: gets output bit set for '1' or '4' 
- **D cube**: gets output bit set for '2', '-', or '~' (don't-care)
- **R cube**: gets output bit set for '3' or '0' (OFF)

Currently, we only create F cubes. We need to split based on PLAType.

---

## Architectural Changes Required

### 1. Change Cube Storage to HashMap

**Rationale**: Need to easily merge cubes with same inputs but different outputs.

#### CoverBuilder Changes
```rust
// OLD:
pub struct CoverBuilder<const INPUTS: usize, const OUTPUTS: usize> {
    cubes: Vec<(Arc<[Option<bool>]>, Arc<[Option<bool>]>)>,
}

// NEW:
pub struct CoverBuilder<const INPUTS: usize, const OUTPUTS: usize, const COVER_TYPE: PLAType = PLAType::F> {
    cubes: HashMap<Arc<[Option<bool>]>, Arc<[Option<bool>]>>,
}
```

**Key changes**:
- Use `HashMap` instead of `Vec` for automatic input-based deduplication
- Add `COVER_TYPE` as const generic parameter (defaults to F for backward compatibility)
- Implement `cover_type()` to return `COVER_TYPE`

#### PLACover Changes
```rust
// OLD:
pub struct PLACover {
    num_inputs: usize,
    num_outputs: usize,
    cubes: Vec<(Vec<Option<bool>>, Vec<Option<bool>>)>,
    pla_type: PLAType,
}

// NEW:
pub struct PLACover {
    num_inputs: usize,
    num_outputs: usize,
    cubes: HashMap<Vec<Option<bool>>, Vec<Option<bool>>>,
    cover_type: PLAType,  // Renamed from pla_type for consistency
}
```

**Key changes**:
- Use `HashMap` instead of `Vec`
- Rename `pla_type` to `cover_type` for consistency with trait
- Parse `.type` directive to set cover_type

### 2. Implement set_cubes_from_iter() for Both Types

#### For CoverBuilder
```rust
fn set_cubes_from_iter(
    &mut self,
    cubes: impl Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)>,
) {
    self.cubes.clear();
    for (inputs, outputs) in cubes {
        let input_arc: Arc<[Option<bool>]> = Arc::from(inputs.into_boxed_slice());
        let output_arc: Arc<[Option<bool>]> = Arc::from(outputs.into_boxed_slice());
        
        // HashMap automatically handles merging by key
        self.cubes.insert(input_arc, output_arc);
    }
}
```

#### For PLACover
```rust
fn set_cubes_from_iter(
    &mut self,
    cubes: impl Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)>,
) {
    self.cubes.clear();
    for (inputs, outputs) in cubes {
        // HashMap automatically handles merging by key
        self.cubes.insert(inputs, outputs);
    }
}
```

### 3. Update cubes_iter() for HashMap

#### For CoverBuilder
```rust
fn cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)> + 'a> {
    Box::new(
        self.cubes
            .iter()
            .map(|(inp, out)| (inp.to_vec(), out.to_vec()))
    )
}
```

#### For PLACover
```rust
fn cubes_iter<'a>(&'a self) -> Box<dyn Iterator<Item = (Vec<Option<bool>>, Vec<Option<bool>>)> + 'a> {
    Box::new(
        self.cubes
            .iter()
            .map(|(inp, out)| (inp.clone(), out.clone()))
    )
}
```

### 4. Update minimize_with_config to Use cover_type()

**Current code** (lines ~310-345) only creates F cubes. **Need to change**:

```rust
fn minimize_with_config(&mut self, config: &EspressoConfig) -> io::Result<()> {
    use worker::Worker;

    let ipc_config = ipc::IpcConfig { /* ... */ };
    let cover_type = self.cover_type();

    // Split cubes based on cover type
    let mut f_cubes = Vec::new();
    let mut d_cubes = Vec::new();
    let mut r_cubes = Vec::new();

    for (inputs, outputs) in self.cubes_iter() {
        let input_vec: Vec<u8> = inputs
            .iter()
            .map(|&opt| match opt {
                Some(false) => 0,
                Some(true) => 1,
                None => 2,
            })
            .collect();

        // Split outputs into F, D, R based on cover_type
        let mut f_output = vec![0; outputs.len()];
        let mut d_output = vec![0; outputs.len()];
        let mut r_output = vec![0; outputs.len()];

        for (i, &opt) in outputs.iter().enumerate() {
            match opt {
                Some(true) if cover_type.has_f() => f_output[i] = 1,
                None if cover_type.has_d() => d_output[i] = 1,
                Some(false) if cover_type.has_r() => r_output[i] = 1,
                _ => {
                    // For F-type: Some(false) and None just don't set any bit
                    // This is correct - they're implicit "not in this cube"
                }
            }
        }

        // Add cubes only if they have bits AND type is enabled
        if f_output.iter().any(|&b| b != 0) {
            f_cubes.push((input_vec.clone(), f_output));
        }
        if d_output.iter().any(|&b| b != 0) {
            d_cubes.push((input_vec.clone(), d_output));
        }
        if r_output.iter().any(|&b| b != 0) {
            r_cubes.push((input_vec, r_output));
        }
    }

    // Call worker with appropriate sets
    let serialized = Worker::execute_minimize(
        self.num_inputs(),
        self.num_outputs(),
        ipc_config,
        f_cubes,
        if d_cubes.is_empty() { None } else { Some(d_cubes) },
        if r_cubes.is_empty() { None } else { Some(r_cubes) },
    )?;

    self.set_cubes_from_worker(&serialized);
    Ok(())
}
```

### 5. Update PLA Parser to Set cover_type Correctly

In `PLACover::from_pla_content()`, after parsing `.type` directive:

```rust
// Parse .type directive (already done, just ensure it's set)
Some(".type") => {
    if let Some(type_str) = parts.get(1) {
        pla_type = match *type_str {
            "f" => PLAType::F,
            "fd" => PLAType::FD,
            "fr" => PLAType::FR,
            "fdr" => PLAType::FDR,
            _ => PLAType::F,
        };
    }
}

// Then at the end, set cover_type field
Ok(PLACover {
    num_inputs: ni,
    num_outputs: no,
    cubes: merged_cubes.into_iter().collect(), // HashMap!
    cover_type: pla_type,  // Use parsed type
})
```

### 6. Update to_pla_string() to Handle Don't-Cares in Outputs

Currently (lines ~330-350), `to_pla_string()` uses `~` for `Some(false)` in FD types. This is WRONG.

**Correct encoding** (matching C code cvrout.c):
- `Some(true)` → `'1'`
- `Some(false)` → `'0'` for F/FR types, `'~'` for FD/FDR types
- `None` → `'-'` always (but shouldn't appear in minimized output)

```rust
let use_tilde = matches!(pla_type, PLAType::FD | PLAType::FDR);
for out in outputs {
    output.push(match out {
        Some(true) => '1',
        Some(false) => if use_tilde { '~' } else { '0' },
        None => '-',  // Shouldn't appear after minimization
    });
}
```

---

## Implementation Steps

### Step 1: Update CoverBuilder Structure
1. Change `cubes` field from `Vec` to `HashMap`
2. Add `const COVER_TYPE: PLAType = PLAType::F` generic parameter
3. Update `new()`, `add_cube()`, `num_cubes()`, `cubes()` methods
4. Implement `cover_type()` → return `COVER_TYPE`
5. Implement `set_cubes_from_iter()` as shown above
6. Update `cubes_iter()` for HashMap

### Step 2: Update PLACover Structure
1. Change `cubes` field from `Vec` to `HashMap`
2. Rename `pla_type` to `cover_type`
3. Update all methods that access cubes
4. Implement `cover_type()` → return `self.cover_type`
5. Implement `set_cubes_from_iter()` as shown above
6. Update `cubes_iter()` for HashMap
7. Update PLA parser to populate HashMap directly
8. Fix `.type` parsing to set cover_type field

### Step 3: Remove Old Decode Functions
1. Delete `decode_serialized_cover` from CoverBuilder impl (now in trait)
2. Delete `decode_serialized_cover` from PLACover impl (now in trait)
3. Delete `set_cubes_from_worker` from both (now uses default trait impl)

### Step 4: Update minimize_with_config
1. Add `let cover_type = self.cover_type();` at top
2. Update cube splitting logic to check `cover_type.has_f()`, `has_d()`, `has_r()`
3. Only create F/D/R cubes based on type
4. Pass D and R to worker only if type supports them

### Step 5: Update Tests
1. Fix `test_programmatic_via_pla.rs` - XOR test now expects HashMap merging
2. Update any tests that create cubes manually
3. Add tests for FD-type PLAs

### Step 6: Run Regression Tests
Expected result: All 28 tests should pass!

---

## Testing Strategy

### Unit Tests
```bash
cargo test --lib  # Should pass all 15 tests
cargo test        # Should pass all 39 tests including integration
```

### Regression Tests
```bash
./tests/regression_test.sh
```

Expected: 28/28 PASS

### Manual Verification
```bash
# F-type (should already work)
./target/release/espresso examples/ex5

# FD-type (currently broken, should work after refactor)
./target/release/espresso examples/ex5 -o fd

# Compare outputs
diff <(./bin/espresso examples/ex5_fd) <(./target/release/espresso examples/ex5_fd)
```

---

## Key Insights from C Code

### From cvrin.c (lines 176-199):
- Each PLA line can create **up to 3 cubes** with same inputs
- Output character mapping:
  - `'1'` or `'4'` → F cube (bit set in ON-set)
  - `'2'`, `'-'`, or `'~'` → D cube (bit set in don't-care set)
  - `'3'` or `'0'` → R cube (bit set in OFF-set)
- Cubes only added to F/D/R if `pla_type & F_type/D_type/R_type`

### From cvrin.c (lines 556-570):
- After parsing, C code computes missing sets:
  - F-type: `R = complement(F, D)` if needed
  - FR-type: `D = complement(F, R)` if needed
  - R-type: `F = complement(D, R)`
- Our worker process does this automatically!

### From cvrout.c:
- Output encoding in PLA files:
  - F/FR types: `'1'` for ON, `'0'` for OFF
  - FD/FDR types: `'1'` for ON, `'~'` for don't-care/OFF
- The `'~'` is only for display in FD/FDR types!

---

## Common Pitfalls to Avoid

1. **Don't split F-type cubes into R cubes**: For F-type, `Some(false)` means "this output bit is not active in this cube", NOT "this cube belongs in OFF-set"

2. **HashMap insertion order**: The order doesn't matter for correctness, but tests might need adjustment if they expect specific ordering

3. **Arc vs Vec in HashMap**: CoverBuilder uses Arc for efficiency, PLACover uses Vec. Keep this distinction.

4. **Const generics default values**: `CoverBuilder<2, 1>` should still work (defaults to F-type)

5. **PLA file `.type` directive**: If missing, defaults to F-type

6. **Don't-care in outputs after minimization**: Should be `Some(false)` (OFF), not `None`, because minimization resolves don't-cares

---

## Expected Final State

After completing this refactoring:
- ✅ All 39 unit tests pass
- ✅ All 28 regression tests pass
- ✅ Outputs identical to C reference implementation
- ✅ Clean architecture with HashMap-based storage
- ✅ Cover type properly parameterized
- ✅ Deserialization logic in trait (DRY principle)
- ✅ Support for F, FD, FR, and FDR PLA types

---

## Files to Modify

1. **src/lib.rs** (main changes):
   - Cover trait (already updated with decode logic)
   - CoverBuilder struct and impl
   - PLACover struct and impl
   - minimize_with_config function
   - to_pla_string function

2. **Tests** (minor updates):
   - tests/test_programmatic_via_pla.rs
   - Possibly update expected cube counts due to HashMap merging

3. **No changes needed**:
   - src/worker.rs (already handles F/D/R cubes correctly)
   - src/ipc.rs (serialization format unchanged)
   - src/bin/espresso.rs (CLI already correct)

---

## Current Code Snapshot

### What's Already Done
- ✅ PLAType helper methods (has_f, has_d, has_r) - lines 151-166
- ✅ decode_serialized_cover moved to Cover trait - lines 254-302
- ✅ set_cubes_from_worker default impl in trait - lines 244-251
- ✅ cover_type() method added to trait - line 230
- ✅ set_cubes_from_iter() method added to trait - lines 238-242

### What's Left
- ❌ Change CoverBuilder to HashMap with COVER_TYPE const generic
- ❌ Change PLACover to HashMap with cover_type field
- ❌ Implement set_cubes_from_iter() for both types
- ❌ Update cubes_iter() for HashMap
- ❌ Update minimize_with_config to use cover_type()
- ❌ Remove old decode functions from impls
- ❌ Update tests

---

## Summary

This refactoring will:
1. Fix all 19 failing FD-type regression tests
2. Provide clean HashMap-based architecture for automatic cube merging
3. Properly parameterize cover type (F/FD/FR/FDR)
4. Move shared deserialization logic to trait (DRY)
5. Make the codebase match the C implementation's behavior exactly

The key insight: We need to split cubes into F/D/R sets **based on the cover type**, and each PLA line with mixed outputs creates multiple orthogonal cubes with the same inputs but different output bits set in different sets (F/D/R).



