//! Comprehensive benchmark suite for Espresso logic minimization
//!
//! This benchmark tests performance across all available PLA test files,
//! categorized by size and complexity.
//!
//! For efficiency, randomly selects 10 files from each size category.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use espresso_logic::{Minimizable, PlaCover, Symbol};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Category of PLA test files
#[derive(Debug, Clone, Copy)]
enum Category {
    Small,     // < 10 cubes
    Medium,    // 10-100 cubes
    Large,     // 100-1000 cubes
    VeryLarge, // > 1000 cubes
}

impl Category {
    fn as_str(&self) -> &str {
        match self {
            Category::Small => "small",
            Category::Medium => "medium",
            Category::Large => "large",
            Category::VeryLarge => "very_large",
        }
    }
}

/// Information about a PLA test file
#[derive(Debug, Clone)]
struct PLATestFile {
    path: PathBuf,
    name: String,
    directory: String,
    category: Category,
    num_cubes: usize,
}

/// Discover all PLA files in the given directories
fn discover_pla_files() -> Vec<PLATestFile> {
    let mut files = Vec::new();

    // Directories to search
    let search_dirs = vec!["examples", "tlex", "hard_examples"];

    for dir in search_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Check if it's a PLA file (has .pla extension or no extension)
                let is_pla = path.extension().map(|ext| ext == "pla").unwrap_or(true)
                    && path.is_file()
                    && !path.file_name().unwrap().to_str().unwrap().ends_with(".rs");

                if is_pla {
                    // Try to parse the file to get cube count
                    if let Ok(cover) = PlaCover::<Symbol>::from_pla_file(&path) {
                        let num_cubes = cover.num_cubes();

                        // Categorize by size
                        let category = if num_cubes < 10 {
                            Category::Small
                        } else if num_cubes < 100 {
                            Category::Medium
                        } else if num_cubes < 1000 {
                            Category::Large
                        } else {
                            Category::VeryLarge
                        };

                        let name = path.file_name().unwrap().to_str().unwrap().to_string();

                        files.push(PLATestFile {
                            path: path.clone(),
                            name,
                            directory: dir.to_string(),
                            category,
                            num_cubes,
                        });
                    }
                }
            }
        }
    }

    // Sort by category and then by number of cubes
    files.sort_by_key(|f| (f.category as u32, f.num_cubes));

    files
}

/// Select up to N files randomly from each category for balanced benchmarking
fn select_balanced_files(files: Vec<PLATestFile>, per_category: usize) -> Vec<PLATestFile> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Group files by category
    let mut by_category: HashMap<u32, Vec<PLATestFile>> = HashMap::new();
    for file in files {
        by_category
            .entry(file.category as u32)
            .or_default()
            .push(file);
    }

    let mut selected = Vec::new();

    // For each category, select up to per_category files
    // Use a deterministic "shuffle" based on filename hash for reproducibility
    for (_, mut category_files) in by_category {
        // Sort by hash of path for deterministic pseudo-random selection
        category_files.sort_by_key(|f| {
            let mut hasher = DefaultHasher::new();
            f.path.hash(&mut hasher);
            hasher.finish()
        });

        // Take up to per_category files
        selected.extend(category_files.into_iter().take(per_category));
    }

    // Sort by category and size for consistent benchmark ordering
    selected.sort_by_key(|f| (f.category as u32, f.num_cubes));

    selected
}

/// Benchmark: Parse PLA file from string
fn bench_parse(c: &mut Criterion) {
    let all_files = discover_pla_files();

    if all_files.is_empty() {
        eprintln!("Warning: No PLA files found for benchmarking");
        return;
    }

    // Select 10 files from each category for balanced benchmarking
    let files = select_balanced_files(all_files, 10);

    eprintln!("Benchmarking {} files for parsing", files.len());

    let mut group = c.benchmark_group("parse_pla");

    for file in files.iter() {
        let param = format!(
            "{}/{}/{}",
            file.category.as_str(),
            file.directory,
            file.name
        );

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        group.bench_with_input(
            BenchmarkId::new("from_file", &param),
            &file.path,
            |b, path| {
                b.iter(|| {
                    let cover = PlaCover::<Symbol>::from_pla_file(black_box(path)).unwrap();
                    black_box(cover);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Minimize PLA covers
fn bench_minimize(c: &mut Criterion) {
    let all_files = discover_pla_files();

    if all_files.is_empty() {
        eprintln!("Warning: No PLA files found for benchmarking");
        return;
    }

    // Select 10 files from each category for balanced benchmarking
    let files = select_balanced_files(all_files, 10);

    eprintln!("Benchmarking {} files for minimization", files.len());

    let mut group = c.benchmark_group("minimize");

    for file in files.iter() {
        let param = format!(
            "{}/{}/{}",
            file.category.as_str(),
            file.directory,
            file.name
        );

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        group.bench_with_input(
            BenchmarkId::new("espresso", &param),
            &file.path,
            |b, path| {
                b.iter(|| {
                    let cover = PlaCover::<Symbol>::from_pla_file(black_box(path)).unwrap();
                    let cover = cover.minimize().unwrap();
                    black_box(cover);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Full pipeline (parse + minimize)
fn bench_full_pipeline(c: &mut Criterion) {
    let all_files = discover_pla_files();

    if all_files.is_empty() {
        eprintln!("Warning: No PLA files found for benchmarking");
        return;
    }

    // Select 10 files from each category for balanced benchmarking
    let files = select_balanced_files(all_files, 10);

    eprintln!("Benchmarking {} files for full pipeline", files.len());

    let mut group = c.benchmark_group("full_pipeline");

    for file in files.iter() {
        let param = format!(
            "{}/{}/{}",
            file.category.as_str(),
            file.directory,
            file.name
        );

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        group.bench_with_input(
            BenchmarkId::new("parse_and_minimize", &param),
            &file.path,
            |b, path| {
                b.iter(|| {
                    let cover = PlaCover::<Symbol>::from_pla_file(black_box(path)).unwrap();
                    let cover = cover.minimize().unwrap();
                    let result = cover.num_cubes();
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark by category - shows how performance scales with problem size
fn bench_by_category(c: &mut Criterion) {
    let files = discover_pla_files();

    if files.is_empty() {
        eprintln!("Warning: No PLA files found for benchmarking");
        return;
    }

    let mut group = c.benchmark_group("by_category");

    // Pick representative examples from each category
    for category in [
        Category::Small,
        Category::Medium,
        Category::Large,
        Category::VeryLarge,
    ] {
        if let Some(file) = files
            .iter()
            .find(|f| matches!(f.category, cat if cat as u32 == category as u32))
        {
            let param = format!("{}/{}", file.directory, file.name);

            group.throughput(Throughput::Elements(file.num_cubes as u64));
            group.bench_with_input(
                BenchmarkId::new(category.as_str(), &param),
                &file.path,
                |b, path| {
                    b.iter(|| {
                        let cover = PlaCover::<Symbol>::from_pla_file(black_box(path)).unwrap();
                        let cover = cover.minimize().unwrap();
                        black_box(cover);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark: Cube iteration performance
fn bench_cube_iteration(c: &mut Criterion) {
    let files = discover_pla_files();

    if files.is_empty() {
        return;
    }

    let mut group = c.benchmark_group("cube_iteration");

    // Test with a medium-sized file
    if let Some(file) = files
        .iter()
        .find(|f| matches!(f.category, Category::Medium))
    {
        let cover = PlaCover::<Symbol>::from_pla_file(&file.path).unwrap();

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        // `PlaCover` is a sum type over which sides are named; match once to get a concrete cover to
        // iterate. (These bench PLAs carry both label sections.)
        group.bench_function("iterate_cubes", |b| {
            b.iter(|| {
                let mut count = 0;
                macro_rules! count_cubes {
                    ($c:expr) => {
                        for cube in $c.cubes() {
                            black_box(cube);
                            count += 1;
                        }
                    };
                }
                match &cover {
                    PlaCover::InputsOutputsNamed(c) => count_cubes!(c),
                    PlaCover::InputsNamed(c) => count_cubes!(c),
                    PlaCover::OutputsNamed(c) => count_cubes!(c),
                    PlaCover::Positional(c) => count_cubes!(c),
                }
                black_box(count);
            });
        });
    }

    group.finish();
}

/// Isolate the eager identity-sort done by `Minterm::labeled` for **named** tables (anonymous
/// tables skip it), plus the value packing that goes with it. Pairs are built once; each iteration
/// measures the sort + table construction + value packing.
fn bench_minterm_labeled_named(c: &mut Criterion) {
    use espresso_logic::Minterm;

    let mut group = c.benchmark_group("minterm_labeled_named");
    for &width in &[16usize, 64, 256] {
        // Reverse-ordered names, so the sort genuinely permutes (not an already-sorted fast path).
        let pairs: Vec<(Symbol, Option<bool>)> = (0..width)
            .rev()
            .map(|i| (Symbol::from(format!("v{i:04}").as_str()), Some(true)))
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(width), &pairs, |b, pairs| {
            b.iter(|| {
                let minterm = Minterm::labeled(pairs).unwrap();
                black_box(minterm);
            });
        });
    }
    group.finish();
}

/// Realistic named lifecycle: construct two differently-ordered named headers, then *use* them —
/// a name-aligned merge-join (`is_subset_of`) plus name lookups (`value_of`). This is where eager and
/// lazy `Symbols` actually differ: lazy defers the sort to the first alignment and builds a HashMap on
/// the first lookup, so a fair comparison must include the alignment/lookups, not just construction.
fn bench_named_align(c: &mut Criterion) {
    use espresso_logic::Minterm;

    let mut group = c.benchmark_group("named_align");
    for &width in &[16usize, 64] {
        let vals: Vec<Option<bool>> = (0..width)
            .map(|i| if i % 2 == 0 { Some(true) } else { None })
            .collect();
        let pairs_a: Vec<(Symbol, Option<bool>)> = (0..width)
            .map(|i| (Symbol::from(format!("v{i:04}").as_str()), vals[i]))
            .collect();
        // Values are assigned by *position* (matching the old `from_symbols(sb, vals.iter()...)`
        // packing), not by the name embedded in the reversed label, so position `j` (not the name's
        // index `i`) gets `vals[j]`.
        let pairs_b: Vec<(Symbol, Option<bool>)> = (0..width)
            .rev()
            .zip(vals.iter().copied())
            .map(|(i, v)| (Symbol::from(format!("v{i:04}").as_str()), v))
            .collect();
        let probes = [
            format!("v{:04}", 0),
            format!("v{:04}", width / 2),
            format!("v{:04}", width - 1),
        ];
        group.bench_with_input(BenchmarkId::from_parameter(width), &width, |b, _| {
            b.iter(|| {
                // Fresh minterms each iter so construction is included alongside the use.
                let ma = Minterm::labeled(&pairs_a).unwrap();
                let mb = Minterm::labeled(&pairs_b).unwrap();
                black_box(ma.is_subset_of(&mb)); // merge-join => sorted_order on both
                for p in &probes {
                    black_box(ma.value_of(p.as_str())); // => index_of
                }
            });
        });
    }
    group.finish();
}

/// Stress the **real** conversion machinery on real PLA inputs: `Cover -> exprs` (`to_exprs`) and
/// `exprs -> Cover` (`add_expr`), round-tripped. Unlike the synthetic `named_align` microbench, this is
/// an actual usage path, and it shows how small a slice `Symbols` construction/lookup is of the real
/// work (BDD build + factorisation dominate) — the honest way to weigh eager vs lazy.
fn bench_pla_expr_roundtrip(c: &mut Criterion) {
    use espresso_logic::{BoolExpr, Cover, CoverType};

    let files = discover_pla_files();
    // `to_exprs` needs named outputs, so use the fully-named (`.ilb` + `.ob`) small/medium files.
    let named: Vec<(String, Cover<Symbol, Symbol>)> = files
        .iter()
        .filter(|f| matches!(f.category, Category::Small | Category::Medium))
        .filter_map(|f| match PlaCover::<Symbol>::from_pla_file(&f.path) {
            Ok(PlaCover::InputsOutputsNamed(cover)) => Some((f.name.clone(), cover)),
            _ => None,
        })
        .take(6)
        .collect();

    if named.is_empty() {
        return;
    }

    let mut group = c.benchmark_group("pla_expr_roundtrip");
    for (name, cover) in &named {
        group.bench_with_input(BenchmarkId::from_parameter(name), cover, |b, cover| {
            b.iter(|| {
                // Cover -> BoolExprs (one per output).
                let exprs: Vec<(Symbol, BoolExpr)> =
                    cover.to_exprs().map(|(n, e)| (n.clone(), e)).collect();
                // BoolExprs -> fresh named Cover (builds named Symbols, unions headers, re-points cubes).
                let mut rebuilt = Cover::<Symbol, Symbol>::new(CoverType::F);
                for (n, e) in &exprs {
                    rebuilt.add_expr(e, n.as_ref()).unwrap();
                }
                black_box(rebuilt);
            });
        });
    }
    group.finish();
}

/// Benchmark: high-level `Cover` API vs low-level `EspressoCover` API, head-to-head.
///
/// Parsing and input extraction happen OUTSIDE the timed region, so each series measures only its
/// layer's minimisation path. We restrict to the parsed cover's ON-set (`F`) cubes and run an F-type
/// minimisation: the simple low-level `from_cubes` path models a single ON-set family, so this makes
/// both layers do exactly the same algorithmic work (both compute the OFF-set complement internally).
/// A one-time fairness check asserts the two layers minimise to the same cube count before a file is
/// included, so any timing delta is pure API/path difference. Three series per file:
///
/// - `high_level` — `Cover::minimize()` on a pre-built cover (validation + word-copy marshal + result
///   decode), i.e. what a high-level user already holds and calls.
/// - `low_level_raw` — `EspressoCover::from_cubes(..).minimize(None, None)`, no result decode: the
///   "maximum performance" path the docs tout, given its best case.
/// - `low_level_decoded` — the raw path plus `to_cubes`, so both layers yield equivalent output;
///   isolates whether any gap is real overhead or just the skipped decode.
fn bench_api_overhead(c: &mut Criterion) {
    use espresso_logic::espresso::EspressoCover;
    use espresso_logic::{Anonymous, Cover, CoverType, Cube, CubeType};

    /// Parse-excluded inputs for one file, shared by all three series.
    struct Prepared {
        param: String,
        num_cubes: usize,
        ni: usize,
        no: usize,
        /// The high-level cover, pre-built: the F-type ON-set the user would hold and `.minimize()`.
        cover: Cover<Anonymous, Anonymous>,
        /// The same ON-set in the low-level `from_cubes` u8 encoding (inputs 0/1/2, outputs 0/1).
        low: Vec<(Vec<u8>, Vec<u8>)>,
    }

    let files = select_balanced_files(discover_pla_files(), 5);
    if files.is_empty() {
        eprintln!("Warning: No PLA files found for api_overhead benchmark");
        return;
    }

    let mut prepared: Vec<Prepared> = Vec::new();
    for file in &files {
        // VeryLarge covers are dominated by minimisation (overhead is within noise) and slow to bench
        // repeatedly across three series; skip them so the run stays tractable.
        if matches!(file.category, Category::VeryLarge) {
            continue;
        }
        let Ok(parsed) = PlaCover::<Symbol>::from_pla_file(&file.path) else {
            continue;
        };
        let parsed = parsed.into_anonymous();
        let ni = parsed.num_inputs();
        let no = parsed.num_outputs();

        // Extract the ON-set (F) cubes once, in both representations.
        let mut cover = Cover::<Anonymous, Anonymous>::anonymous(CoverType::F);
        let mut low: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for cube in parsed.cubes() {
            if cube.cube_type() != CubeType::F {
                continue;
            }
            let inputs: Vec<Option<bool>> = cube.inputs().iter().collect();
            let membership: Vec<bool> = cube.outputs().iter().collect();
            let low_in: Vec<u8> = inputs
                .iter()
                .map(|v| match v {
                    Some(false) => 0,
                    Some(true) => 1,
                    None => 2,
                })
                .collect();
            let low_out: Vec<u8> = membership.iter().map(|&b| u8::from(b)).collect();
            cover.push(Cube::anonymous(&inputs, &membership, CubeType::F));
            low.push((low_in, low_out));
        }
        if low.is_empty() {
            continue;
        }

        // Fairness check (one-time, outside timing): both layers must minimise to the same cube count,
        // otherwise the timed comparison would not be apples-to-apples.
        let hi_count = cover.minimize().ok().map(|c| c.num_cubes());
        let refs: Vec<(&[u8], &[u8])> = low
            .iter()
            .map(|(a, b)| (a.as_slice(), b.as_slice()))
            .collect();
        let lo_count = EspressoCover::from_cubes(&refs, ni, no).ok().map(|cover| {
            let (f, _d, _r) = cover.minimize(None, None);
            f.to_cubes(ni, no, CubeType::F).len()
        });
        if hi_count.is_none() || hi_count != lo_count {
            eprintln!(
                "api_overhead: skipping {} (fairness check failed: high={hi_count:?} low={lo_count:?})",
                file.name
            );
            continue;
        }
        drop(refs);

        prepared.push(Prepared {
            param: format!(
                "{}/{}/{}",
                file.category.as_str(),
                file.directory,
                file.name
            ),
            num_cubes: file.num_cubes,
            ni,
            no,
            cover,
            low,
        });
    }

    if prepared.is_empty() {
        eprintln!("Warning: api_overhead found no usable files");
        return;
    }
    eprintln!(
        "Benchmarking {} files for api_overhead (high-level vs low-level)",
        prepared.len()
    );

    let mut group = c.benchmark_group("api_overhead");
    // Three series over non-trivial covers is heavier than the single-series groups; trim the sample
    // size so the whole group stays within a reasonable wall-clock budget.
    group.sample_size(20);

    for p in &prepared {
        group.throughput(Throughput::Elements(p.num_cubes as u64));

        group.bench_with_input(BenchmarkId::new("high_level", &p.param), p, |b, p| {
            b.iter(|| {
                let out = black_box(&p.cover).minimize().unwrap();
                black_box(out);
            });
        });

        group.bench_with_input(BenchmarkId::new("low_level_raw", &p.param), p, |b, p| {
            b.iter(|| {
                let refs: Vec<(&[u8], &[u8])> = p
                    .low
                    .iter()
                    .map(|(a, b)| (a.as_slice(), b.as_slice()))
                    .collect();
                let cover = EspressoCover::from_cubes(black_box(&refs), p.ni, p.no).unwrap();
                let (f, _d, _r) = cover.minimize(None, None);
                black_box(f);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("low_level_decoded", &p.param),
            p,
            |b, p| {
                b.iter(|| {
                    let refs: Vec<(&[u8], &[u8])> = p
                        .low
                        .iter()
                        .map(|(a, b)| (a.as_slice(), b.as_slice()))
                        .collect();
                    let cover = EspressoCover::from_cubes(black_box(&refs), p.ni, p.no).unwrap();
                    let (f, _d, _r) = cover.minimize(None, None);
                    // Consume the lazy iterator so every cube is actually decoded inside the timed loop.
                    f.to_cubes(p.ni, p.no, CubeType::F).for_each(|c| {
                        black_box(c);
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_minimize,
    bench_full_pipeline,
    bench_by_category,
    bench_cube_iteration,
    bench_minterm_labeled_named,
    bench_named_align,
    bench_pla_expr_roundtrip,
    bench_api_overhead
);
criterion_main!(benches);
