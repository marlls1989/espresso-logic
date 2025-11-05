//! Espresso Logic Minimizer - Command Line Interface
//!
//! A clean Rust implementation using the safe Rust API

use clap::{Parser, ValueEnum};
use espresso_logic::{EspressoConfig, PLAType, PLA};
use std::path::PathBuf;
use std::process;

const VERSION: &str =
    "UC Berkeley, Espresso Version #2.3, Release date 01/31/88 (Rust wrapper 2.3.0)";

#[derive(Debug, Clone, ValueEnum)]
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

impl From<OutputType> for PLAType {
    fn from(val: OutputType) -> Self {
        match val {
            OutputType::F => PLAType::F,
            OutputType::Fd => PLAType::FD,
            OutputType::Fr => PLAType::FR,
            OutputType::Fdr => PLAType::FDR,
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
}

fn main() {
    let args = Args::parse();

    // Create and apply configuration using safe API
    let config = EspressoConfig {
        summary: args.summary,
        ..Default::default()
    };
    config.apply();

    if args.summary {
        eprintln!("{}", VERSION);
        eprintln!();
    }

    // Read the input PLA using our safe Rust API
    let pla = match PLA::from_file(&args.input) {
        Ok(pla) => pla,
        Err(e) => {
            eprintln!("Error reading PLA file '{}': {}", args.input.display(), e);
            process::exit(1);
        }
    };

    if args.summary {
        eprintln!("Input PLA: {:?}", pla);
        pla.print_summary();
        eprintln!();
    }

    // Execute the command using our safe Rust API
    let result_pla = match args.command {
        Command::Espresso => {
            if args.summary {
                eprintln!("Running Espresso minimization...");
            }
            pla.minimize()
        }
        Command::Exact => {
            if args.summary {
                eprintln!("Running minimization (exact mode)...");
            }
            // For now, both use the same minimize() method
            // In the future, we can expose minimize_exact through the API
            pla.minimize()
        }
        Command::Echo => {
            if args.summary {
                eprintln!("Echoing PLA without modification...");
            }
            pla
        }
        Command::Stats => {
            let stats = pla.stats();
            println!("PLA Statistics:");
            println!("  ON-set cubes:        {}", stats.num_cubes_f);
            println!("  Don't-care cubes:    {}", stats.num_cubes_d);
            println!("  OFF-set cubes:       {}", stats.num_cubes_r);
            if args.no_output {
                process::exit(0);
            }
            pla
        }
    };

    if args.summary {
        eprintln!("Output PLA: {:?}", result_pla);
        result_pla.print_summary();
        eprintln!();
    }

    // Write the output using our safe Rust API
    if !args.no_output {
        let output_type = PLAType::from(args.output_format);

        if let Some(ref output_path) = args.output_file {
            match result_pla.to_file(output_path, output_type) {
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
            // Write to stdout using safe API
            if let Err(e) = result_pla.write_to_stdout(output_type) {
                eprintln!("Error writing to stdout: {}", e);
                process::exit(1);
            }
        }
    }

    if args.summary {
        eprintln!("Done.");
    }
}
