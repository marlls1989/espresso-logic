# Espresso Logic Minimiser - WebAssembly Demo

This is an interactive WebAssembly demonstration of the [espresso-logic](https://crates.io/crates/espresso-logic) Rust library, built with React and Emscripten.

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
- Node.js 18+ ([Install Node.js](https://nodejs.org))
- Emscripten ([Install Emscripten](https://emscripten.org/docs/getting_started/downloads.html))

### Setup

1. Clone this repository and checkout the `gh-pages` branch:

```bash
git clone https://github.com/marlls1989/espresso-logic.git
cd espresso-logic
git checkout gh-pages
```

2. Install Node dependencies:

```bash
npm install
```

3. Add the wasm32-unknown-emscripten target:

```bash
rustup target add wasm32-unknown-emscripten
```

4. Build the WASM module:

```bash
./build-wasm.sh
# Or manually:
cargo build --target wasm32-unknown-emscripten --release
mkdir -p public/wasm
cp target/wasm32-unknown-emscripten/release/espresso_wasm.js public/wasm/
cp target/wasm32-unknown-emscripten/release/espresso_wasm.wasm public/wasm/
```

5. Start the development server:

```bash
npm run dev
```

6. Open your browser to `http://localhost:5173`

### Production Build

```bash
./build-wasm.sh
npm run build
```

The output will be in the `dist/` directory.

## 🏗️ Architecture

### Technology Stack

- **Frontend**: React 18 for UI components and state management
- **Build Tool**: Vite for fast development and optimised builds
- **WebAssembly**: Emscripten-compiled Rust (wasm32-unknown-emscripten target)
- **Rust Library**: espresso-logic v3.1.3 with Emscripten support

### Project Structure

```
.
├── src/
│   ├── lib.rs                      # Rust FFI bindings for JavaScript
│   ├── main.jsx                    # React entry point
│   ├── App.jsx                     # Main React app component
│   ├── styles.css                  # Global styles
│   └── components/
│       ├── CoverTypeSelector.jsx   # Cover type dropdown
│       ├── EditorPanel.jsx         # Expression input editor
│       ├── ExamplesSelector.jsx    # Example presets
│       ├── ResultsPanel.jsx        # Output display
│       └── TruthTable.jsx          # Cube display table
├── Cargo.toml                      # Rust dependencies
├── package.json                    # Node dependencies
├── vite.config.js                  # Vite configuration
├── build-wasm.sh                   # WASM build script
├── index.html                      # Entry point
└── .github/workflows/deploy.yml    # Auto-deployment

```

### How It Works

1. **Rust Side** - The `src/lib.rs` file exposes C-compatible FFI functions:
   - `minimise_expressions()` - Takes input text and cover type, returns JSON results
   - `free_string()` - Frees strings allocated by Rust

2. **Emscripten** - Compiles Rust + C code to WebAssembly:
   - Handles the original Espresso C code (requires libc functions)
   - Generates JS glue code for calling Rust functions
   - Provides memory management utilities

3. **React Side** - The `App.jsx` component:
   - Loads the Emscripten module on startup
   - Calls Rust functions via the Module interface
   - Displays results using React components

### Key Components

1. **App** (`src/App.jsx`)
   - Loads WASM module
   - Manages application state
   - Calls Rust functions for minimisation

2. **CoverTypeSelector** (`src/components/CoverTypeSelector.jsx`)
   - Dropdown for F/FD/FR/FDR selection
   - Tooltips explaining each type

3. **ExamplesSelector** (`src/components/ExamplesSelector.jsx`)
   - Pre-loaded example expressions
   - Quick-start for users

4. **EditorPanel** (`src/components/EditorPanel.jsx`)
   - Textarea for expression input
   - Minimise button with loading state
   - Error display

5. **ResultsPanel** (`src/components/ResultsPanel.jsx`)
   - Displays minimised expressions
   - Shows statistics
   - Renders truth table

6. **TruthTable** (`src/components/TruthTable.jsx`)
   - Displays cubes with inputs and outputs
   - Colour-coded values (1, 0, don't-care)

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

### Adding Examples

Edit `src/components/ExamplesSelector.jsx`:

```javascript
const EXAMPLES = [
  {
    name: 'Your Example',
    description: 'Description here',
    code: 'out = your + expression',
  },
  // ... more examples
];
```

### Styling

Modify `src/styles.css` (uses CSS custom properties for theming).

## 📦 Deployment

This branch uses GitHub Actions for automatic deployment:

1. Push to `gh-pages` branch
2. GitHub Actions builds WASM and React automatically
3. Deploys to GitHub Pages

Manual deployment:

```bash
./build-wasm.sh
npm run build
# Upload dist/ contents to your web server
```

## 🔗 Links

- **Main Library**: [espresso-logic on crates.io](https://crates.io/crates/espresso-logic)
- **Documentation**: [docs.rs/espresso-logic](https://docs.rs/espresso-logic)
- **GitHub Repository**: [marlls1989/espresso-logic](https://github.com/marlls1989/espresso-logic)
- **Original Espresso**: UC Berkeley EECS Department

## 📄 License

This demo inherits the MIT licence from the espresso-logic library.

## 🙏 Acknowledgements

- **Original Espresso** - Robert K. Brayton and team at UC Berkeley
- **Modernised C Code** - Sébastien Cottinet
- **Rust Wrapper** - Marcos Sartori
- **React** - Meta and contributors
- **Emscripten** - Emscripten contributors

---

Built with ❤️ using Rust, React, and WebAssembly
