# Espresso Logic Minimiser - WebAssembly Demo

This is an interactive WebAssembly demonstration of the [espresso-logic](https://crates.io/crates/espresso-logic) Rust library, built with [Yew](https://yew.rs).

## 🌐 Live Demo

Visit the live demo at: **[https://marlls1989.github.io/espresso-logic/](https://marlls1989.github.io/espresso-logic/)**

## 📋 Features

- **Interactive Expression Editor** - Enter Boolean expressions using standard operators
- **Real-time Minimisation** - See your logic minimised using the Espresso algorithm
- **Truth Table Display** - View the minimised cubes in table format
- **Multiple Outputs** - Define and minimise multiple expressions simultaneously
- **Cover Type Selection** - Choose from F, FD, FR, or FDR cover types
- **Pre-loaded Examples** - XOR, adders, majority functions, and more

## 🚀 Local Development

### Prerequisites

- Rust 1.70+ ([Install Rust](https://rustup.rs))
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

Install wasm-pack:

```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### Building

1. Clone this repository and checkout the `gh-pages` branch:

```bash
git clone https://github.com/marlls1989/espresso-logic.git
cd espresso-logic
git checkout gh-pages
```

2. Build the WebAssembly package:

```bash
wasm-pack build --target web --release
```

3. Serve locally:

```bash
# Using Python
python3 -m http.server 8080

# Or using a simple Rust server
cargo install simple-http-server
simple-http-server -p 8080
```

4. Open your browser to `http://localhost:8080`

### Development Build

For faster iteration during development:

```bash
wasm-pack build --target web --dev
```

## 🏗️ Architecture

### Project Structure

```
.
├── src/
│   ├── lib.rs                      # Main Yew application
│   └── components/
│       ├── mod.rs                  # Component exports
│       ├── cover_type_selector.rs  # Cover type dropdown
│       ├── examples_selector.rs    # Example presets
│       └── truth_table.rs          # Cube display table
├── index.html                      # Entry point
├── styles.css                      # Styling
├── Cargo.toml                      # Dependencies
└── .github/workflows/deploy.yml    # Auto-deployment
```

### Technology Stack

- **[Yew](https://yew.rs)** - Rust framework for building web applications
- **[WebAssembly](https://webassembly.org)** - Binary instruction format for the web
- **[espresso-logic](https://crates.io/crates/espresso-logic)** - Boolean logic minimisation library
- **[wasm-pack](https://rustwasm.github.io/wasm-pack/)** - Build tool for WebAssembly

### Key Components

1. **App Component** (`src/lib.rs`)
   - Main application state management
   - Expression parsing and validation
   - Minimisation pipeline

2. **CoverTypeSelector** (`src/components/cover_type_selector.rs`)
   - Dropdown for F/FD/FR/FDR selection
   - Tooltip explanations for each type

3. **ExamplesSelector** (`src/components/examples_selector.rs`)
   - Pre-loaded example expressions
   - Quick-start for users

4. **TruthTable** (`src/components/truth_table.rs`)
   - Display minimised cubes
   - Shows inputs and outputs in tabular format

## 📖 Usage

### Expression Syntax

- **AND**: `*` or `&`
- **OR**: `+` or `|`
- **NOT**: `~` or `!`
- **Parentheses**: `(` and `)`

### Input Format

Define outputs as `name = expression` (one per line):

```
x = a * b + a * b * c
y = a + b
z = ~a * c
```

### Cover Types

- **F** - ON-set only (specifies where output is 1)
- **FD** - ON-set + Don't-cares (most flexible)
- **FR** - ON-set + OFF-set (specifies both 1s and 0s)
- **FDR** - Complete specification with all sets

## 🔧 Customisation

To modify the examples, edit `src/components/examples_selector.rs`:

```rust
const EXAMPLES: &[Example] = &[
    Example {
        name: "Your Example",
        description: "Description here",
        code: "out = your + expression",
    },
    // ... more examples
];
```

To change styling, modify `styles.css` (uses CSS custom properties for theming).

## 📦 Deployment

This branch uses GitHub Actions for automatic deployment:

1. Push to `gh-pages` branch
2. GitHub Actions builds WebAssembly automatically
3. Deploys to GitHub Pages

Manual deployment:

```bash
wasm-pack build --target web --release
# Upload dist/ contents to your web server
```

## 🔗 Links

- **Main Library**: [espresso-logic on crates.io](https://crates.io/crates/espresso-logic)
- **Documentation**: [docs.rs/espresso-logic](https://docs.rs/espresso-logic)
- **GitHub Repository**: [marlls1989/espresso-logic](https://github.com/marlls1989/espresso-logic)
- **Original Espresso**: UC Berkeley EECS Department

## 📄 License

This demo inherits the MIT license from the espresso-logic library.

## 🙏 Acknowledgements

- **Original Espresso** - Robert K. Brayton and team at UC Berkeley
- **Modernised C Code** - Sébastien Cottinet
- **Rust Wrapper** - Marcos Sartori
- **Yew Framework** - Yew contributors

---

Built with ❤️ using Rust and WebAssembly
