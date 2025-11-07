//! Comprehensive benchmark suite for Espresso logic minimization
//!
//! This benchmark tests performance across all available PLA test files,
//! categorized by size and complexity.
//!
//! For efficiency, randomly selects 10 files from each size category.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use espresso_logic::{Cover, PLACover};
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
                    && !path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .ends_with(".rs");

                if is_pla {
                    // Try to parse the file to get cube count
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(cover) = PLACover::from_pla_content(&content) {
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

                            let name = path
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string();

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
    }

    // Sort by category and then by number of cubes
    files.sort_by(|a, b| {
        (a.category as u32, a.num_cubes).cmp(&(b.category as u32, b.num_cubes))
    });

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
            .or_insert_with(Vec::new)
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
    selected.sort_by(|a, b| {
        (a.category as u32, a.num_cubes).cmp(&(b.category as u32, b.num_cubes))
    });
    
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
        let content = fs::read_to_string(&file.path).unwrap();
        let param = format!("{}/{}/{}", file.category.as_str(), file.directory, file.name);

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        group.bench_with_input(BenchmarkId::new("from_content", &param), &content, |b, data| {
            b.iter(|| {
                let cover = PLACover::from_pla_content(black_box(data)).unwrap();
                black_box(cover);
            });
        });
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
        let content = fs::read_to_string(&file.path).unwrap();
        let param = format!("{}/{}/{}", file.category.as_str(), file.directory, file.name);

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        group.bench_with_input(BenchmarkId::new("espresso", &param), &content, |b, data| {
            b.iter(|| {
                let mut cover = PLACover::from_pla_content(black_box(data)).unwrap();
                cover.minimize().unwrap();
                black_box(cover);
            });
        });
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
        let content = fs::read_to_string(&file.path).unwrap();
        let param = format!("{}/{}/{}", file.category.as_str(), file.directory, file.name);

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        group.bench_with_input(
            BenchmarkId::new("parse_and_minimize", &param),
            &content,
            |b, data| {
                b.iter(|| {
                    let mut cover = PLACover::from_pla_content(black_box(data)).unwrap();
                    cover.minimize().unwrap();
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
        if let Some(file) = files.iter().find(|f| matches!(f.category, cat if cat as u32 == category as u32)) {
            let content = fs::read_to_string(&file.path).unwrap();
            let param = format!("{}/{}", file.directory, file.name);

            group.throughput(Throughput::Elements(file.num_cubes as u64));
            group.bench_with_input(
                BenchmarkId::new(category.as_str(), &param),
                &content,
                |b, data| {
                    b.iter(|| {
                        let mut cover = PLACover::from_pla_content(black_box(data)).unwrap();
                        cover.minimize().unwrap();
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
    if let Some(file) = files.iter().find(|f| matches!(f.category, Category::Medium)) {
        let content = fs::read_to_string(&file.path).unwrap();
        let cover = PLACover::from_pla_content(&content).unwrap();

        group.throughput(Throughput::Elements(file.num_cubes as u64));
        group.bench_function("iterate_cubes", |b| {
            b.iter(|| {
                let mut count = 0;
                for cube in cover.cubes_iter() {
                    black_box(cube);
                    count += 1;
                }
                black_box(count);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_minimize,
    bench_full_pipeline,
    bench_by_category,
    bench_cube_iteration
);
criterion_main!(benches);

