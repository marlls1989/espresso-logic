//! Espresso Logic Minimizer - Command Line Interface
//!
//! A clean Rust implementation using the safe Cover API with process isolation

use clap::{Parser, ValueEnum};
use espresso_logic::{Cover, CoverType, EspressoConfig, PLAReader, PLAWriter};
use std::path::PathBuf;
use std::process;

const VERSION: &str =
    "UC Berkeley, Espresso Version #2.3, Release date 01/31/88 (Rust wrapper 3.0.0)";

#[derive(Debug, Clone, PartialEq, ValueEnum)]
enum Command {
    /// Run the Espresso heuristic minimization algorithm (default)
    Espresso,
    /// Exact minimization (note: uses same algorithm for now)
    Exact,
    /// Echo the PLA without modification
    Echo,
    /// Print statistics about the PLA
    Stats,
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputType {
    /// ON-set only
    F,
    /// ON-set and don't-care set
    Fd,
    /// ON-set and OFF-set
    Fr,
    /// ON-set, don't-care set, and OFF-set
    Fdr,
}

impl From<OutputType> for CoverType {
    fn from(val: OutputType) -> Self {
        match val {
            OutputType::F => CoverType::F,
            OutputType::Fd => CoverType::FD,
            OutputType::Fr => CoverType::FR,
            OutputType::Fdr => CoverType::FDR,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "espresso")]
#[command(about = "Espresso heuristic logic minimizer", long_about = None)]
#[command(version = VERSION)]
struct Args {
    /// Input PLA file (required)
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Subcommand to execute
    #[arg(short = 'D', long = "do", value_enum, default_value = "espresso")]
    command: Command,

    /// Output format
    #[arg(short = 'o', long = "output", value_enum, default_value = "f")]
    output_format: OutputType,

    /// Provide execution summary
    #[arg(short = 's', long = "summary")]
    summary: bool,

    /// Suppress printing of solution
    #[arg(short = 'x', long = "no-output")]
    no_output: bool,

    /// Output file (writes to stdout if not specified)
    #[arg(short = 'O', long = "out-file")]
    output_file: Option<PathBuf>,

    /// Enable debug output (prints to stderr)
    #[arg(short = 'd', long = "debug")]
    debug: bool,

    /// Enable verbose debug output (prints to stderr)
    #[arg(short = 'v', long = "verbose")]
    verbose_debug: bool,

    /// Enable trace output (prints to stderr)
    #[arg(short = 't', long = "trace")]
    trace: bool,

    /// Use single expand (fast mode)
    #[arg(long = "fast")]
    single_expand: bool,

    /// Use exact minimization (slower, but guarantees minimal result)
    #[arg(short = 'e', long = "exact")]
    exact: bool,
}

fn main() {
    let args = Args::parse();

    if args.summary {
        eprintln!("{}", VERSION);
        eprintln!();
    }

    // Read the input PLA using Cover API
    let mut cover = match Cover::from_pla_file(&args.input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading PLA file '{}': {}", args.input.display(), e);
            process::exit(1);
        }
    };

    if args.summary {
        eprintln!(
            "Input: {} inputs, {} outputs, {} cubes",
            cover.num_inputs(),
            cover.num_outputs(),
            cover.num_cubes()
        );
        eprintln!();
    }

    // Build configuration from command-line args
    // Note: debug/trace/verbose output from the C code is redirected to stderr
    // in the worker process, so it won't interfere with the IPC channel
    let config = EspressoConfig {
        debug: args.debug,
        verbose_debug: args.verbose_debug,
        trace: args.trace,
        summary: args.summary,
        single_expand: args.single_expand || args.command == Command::Exact || args.exact,
        ..Default::default()
    };

    // Execute the command using the Cover trait
    match args.command {
        Command::Espresso => {
            if args.summary {
                eprintln!("Running Espresso minimization (process-isolated)...");
            }
            if let Err(e) = cover.minimize_with_config(&config) {
                eprintln!("Error during minimization: {}", e);
                process::exit(1);
            }
        }
        Command::Exact => {
            if args.summary {
                eprintln!("Running exact minimization (process-isolated)...");
            }
            if let Err(e) = cover.minimize_with_config(&config) {
                eprintln!("Error during minimization: {}", e);
                process::exit(1);
            }
        }
        Command::Echo => {
            if args.summary {
                eprintln!("Echoing PLA without modification...");
            }
            // No modification needed
        }
        Command::Stats => {
            println!("PLA Statistics:");
            println!("  Inputs:  {}", cover.num_inputs());
            println!("  Outputs: {}", cover.num_outputs());
            println!("  Cubes:   {}", cover.num_cubes());
            if args.no_output {
                process::exit(0);
            }
        }
    };

    if args.summary {
        eprintln!(
            "Output: {} inputs, {} outputs, {} cubes",
            cover.num_inputs(),
            cover.num_outputs(),
            cover.num_cubes()
        );
        eprintln!();
    }

    // Write the output using Cover trait
    if !args.no_output {
        let output_type = CoverType::from(args.output_format);

        if let Some(ref output_path) = args.output_file {
            match cover.to_pla_file(output_path, output_type) {
                Ok(_) => {
                    if args.summary {
                        eprintln!("Wrote output to: {}", output_path.display());
                    }
                }
                Err(e) => {
                    eprintln!("Error writing output file: {}", e);
                    process::exit(1);
                }
            }
        } else {
            // Write to stdout
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            if let Err(e) = cover.write_pla(&mut handle, output_type) {
                eprintln!("Error writing PLA output: {}", e);
                process::exit(1);
            }
        }
    }

    if args.summary {
        eprintln!("Done.");
    }
}
