import { useState, useEffect } from 'react';
import EditorPanel from './components/EditorPanel';
import ResultsPanel from './components/ResultsPanel';
import { initWasm, callMinimise } from './wasm-loader';

function App() {
  const [wasmReady, setWasmReady] = useState(false);
  const [loading, setLoading] = useState(true);
  const [inputText, setInputText] = useState('x = a * b + a * b * c\ny = a + b');
  const [coverType, setCoverType] = useState(2); // 0=F, 1=FD, 2=FR (default), 3=FDR
  const [result, setResult] = useState(null);
  const [error, setError] = useState(null);
  const [processing, setProcessing] = useState(false);

  useEffect(() => {
    // Initialize WASM module
    initWasm()
      .then(() => {
        setWasmReady(true);
        setLoading(false);
      })
      .catch((err) => {
        setError({ message: 'Failed to load WebAssembly: ' + err.message, line: -1 });
        setLoading(false);
      });
  }, []);

  // Validate input on client side
  const validateInput = (input) => {
    const lines = input.split('\n');

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const trimmedLine = line.trim();
      if (!trimmedLine || trimmedLine.startsWith('#')) continue;

      const errorRegions = [];
      let primaryMessage = '';

      // Check for assignment format
      if (!trimmedLine.includes('=')) {
        return { message: 'Missing assignment operator (=)', line: i, errorRegions: [{ startCol: 0, endCol: line.length }] };
      }

      const eqIndex = line.indexOf('=');
      const name = line.substring(0, eqIndex).trim();
      const expr = line.substring(eqIndex + 1).trim();
      const exprStartCol = eqIndex + 1 + (line.substring(eqIndex + 1).length - expr.length);

      // Check empty name
      if (!name) {
        return { message: 'Empty output name', line: i, errorRegions: [{ startCol: 0, endCol: eqIndex }] };
      }

      // Check empty expression
      if (!expr) {
        return { message: 'Empty expression', line: i, errorRegions: [{ startCol: exprStartCol, endCol: line.length }] };
      }

      // Tokenize the expression and track positions
      const tokenRegex = /[a-zA-Z_][a-zA-Z0-9_]*|[01]|[+|&*~!()]|\S+/g;
      const tokens = [];
      let match;
      while ((match = tokenRegex.exec(expr)) !== null) {
        tokens.push({
          value: match[0],
          start: exprStartCol + match.index,
          end: exprStartCol + match.index + match[0].length
        });
      }

      if (tokens.length === 0) {
        return { message: 'Empty expression', line: i, errorRegions: [{ startCol: exprStartCol, endCol: line.length }] };
      }

      // Valid operators and symbols
      const validOperators = new Set(['+', '|', '&', '*', '~', '!', '(', ')']);
      const validConstants = new Set(['0', '1']);

      // Check all tokens for validity
      for (let j = 0; j < tokens.length; j++) {
        const token = tokens[j];

        // Check if token is valid
        const isValid =
          validOperators.has(token.value) ||
          validConstants.has(token.value) ||
          /^[a-zA-Z_][a-zA-Z0-9_]*$/.test(token.value);

        if (!isValid) {
          errorRegions.push({ startCol: token.start, endCol: token.end });
          if (!primaryMessage) primaryMessage = `Invalid token: '${token.value}'`;
        }
      }

      // Check for missing operators between operands
      for (let j = 0; j < tokens.length - 1; j++) {
        const current = tokens[j];
        const next = tokens[j + 1];

        const isOperand = (tok) =>
          validConstants.has(tok.value) ||
          /^[a-zA-Z_][a-zA-Z0-9_]*$/.test(tok.value) ||
          tok.value === ')';

        const needsOperatorBefore = (tok) =>
          validConstants.has(tok.value) ||
          /^[a-zA-Z_][a-zA-Z0-9_]*$/.test(tok.value) ||
          tok.value === '~' ||
          tok.value === '!' ||
          tok.value === '(';

        // Check for two operands in a row (missing operator)
        // But closing paren after operand is OK
        if (isOperand(current) && needsOperatorBefore(next)) {
          errorRegions.push({ startCol: current.end, endCol: next.start });
          if (!primaryMessage) primaryMessage = 'Missing operator between operands';
        }
      }

      // Check for incomplete expressions (operators at start/end)
      const firstToken = tokens[0];
      const lastToken = tokens[tokens.length - 1];

      if (['+', '|', '&', '*'].includes(firstToken.value)) {
        errorRegions.push({ startCol: Math.max(0, firstToken.start - 1), endCol: firstToken.start });
        if (!primaryMessage) primaryMessage = 'Missing operand before operator';
      }
      if (['+', '|', '&', '*', '~', '!'].includes(lastToken.value)) {
        errorRegions.push({ startCol: lastToken.end, endCol: lastToken.end + 1 });
        if (!primaryMessage) primaryMessage = 'Missing operand after operator';
      }

      // Check for unbalanced parentheses
      let parenCount = 0;
      let errorToken = null;
      for (const token of tokens) {
        if (token.value === '(') parenCount++;
        if (token.value === ')') {
          parenCount--;
          if (parenCount < 0) {
            errorToken = token;
            break;
          }
        }
      }
      if (parenCount < 0 && errorToken) {
        // Highlight the mismatched closing paren itself
        errorRegions.push({ startCol: errorToken.start, endCol: errorToken.end });
        if (!primaryMessage) primaryMessage = 'Unmatched closing parenthesis';
      }
      if (parenCount > 0) {
        // Missing closing paren - highlight space after last token
        const lastTokenEnd = tokens[tokens.length - 1].end;
        errorRegions.push({ startCol: lastTokenEnd, endCol: lastTokenEnd + 1 });
        if (!primaryMessage) primaryMessage = 'Missing closing parenthesis';
      }

      // If we found any errors, return them
      if (errorRegions.length > 0) {
        return {
          message: errorRegions.length > 1 ? `${errorRegions.length} errors found` : primaryMessage,
          line: i,
          errorRegions: errorRegions
        };
      }
    }

    return null;
  };

  // Auto-minimise with debounce - shows errors in real-time
  useEffect(() => {
    if (!wasmReady || !inputText.trim()) {
      setError(null);
      return;
    }

    setProcessing(true);
    const timer = setTimeout(() => {
      // First check client-side validation
      const validationError = validateInput(inputText);
      if (validationError) {
        setError(validationError);
        setProcessing(false);
        return;
      }

      setError(null);

      try {
        const data = callMinimise(inputText, coverType);

        if (data.error) {
          // Show error feedback immediately
          const parsedError = parseError(data.error, inputText);
          setError(parsedError);
          // Don't update result on error - keep previous result
        } else {
          // Only update result on success
          setResult(data);
          setError(null);
        }
      } catch (err) {
        // Show error feedback immediately
        setError({ message: err.message, line: -1 });
        // Don't update result on error - keep previous result
      } finally {
        setProcessing(false);
      }
    }, 500);

    return () => {
      clearTimeout(timer);
      setProcessing(false);
    };
  }, [inputText, coverType, wasmReady]);

  // Parse error message to extract line information
  const parseError = (errorMsg, input) => {
    const lines = input.split('\n');

    // Try to extract line number from new format "Line N: message"
    const lineNumberMatch = errorMsg.match(/^Line (\d+): (.+)$/);
    if (lineNumberMatch) {
      const lineNum = parseInt(lineNumberMatch[1], 10);
      const message = lineNumberMatch[2];

      // Clean up the message
      let cleanMessage = message;

      // Remove redundant expressions in quotes if present
      const inExprMatch = message.match(/^(.+) in expression '([^']+)'$/);
      if (inExprMatch) {
        cleanMessage = inExprMatch[1];
      }

      const inMatch = message.match(/^(.+) in '([^']+)'$/);
      if (inMatch) {
        cleanMessage = inMatch[1];
      }

      // Remove "Invalid format 'line' - expected: ..." to just show the expected part
      const formatMatch = message.match(/^Invalid format '[^']+' - (.+)$/);
      if (formatMatch) {
        cleanMessage = formatMatch[1].charAt(0).toUpperCase() + formatMatch[1].slice(1);
      }

      // Return with errorRegions for backwards compatibility
      return {
        message: cleanMessage,
        line: lineNum - 1, // Convert to 0-based index
        errorRegions: [{ startCol: 0, endCol: lines[lineNum - 1]?.length || 0 }]
      };
    }

    // Fallback: try old formats for backwards compatibility
    const parseErrorMatch = errorMsg.match(/Parse error in '([^']+)':/);
    if (parseErrorMatch) {
      const problematicExpr = parseErrorMatch[1];
      const lineIndex = lines.findIndex(line => line.includes(problematicExpr));
      return {
        message: errorMsg.replace(/^Parse error in '[^']+': /, ''),
        line: lineIndex >= 0 ? lineIndex : -1,
        errorRegions: [{ startCol: 0, endCol: lines[lineIndex]?.length || 0 }]
      };
    }

    return { message: errorMsg, line: -1, errorRegions: [] };
  };

  const handleInputChange = (newText) => {
    setInputText(newText);
  };

  const handleLoadExample = (exampleText) => {
    setInputText(exampleText);
  };

  if (loading) {
    return (
      <div className="app-container">
        <div className="empty-state">
          <div className="empty-state-icon">⏳</div>
          <p>Loading WebAssembly module...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app-container">
      <header>
        <h1>Espresso Logic Minimiser</h1>
        <p>Interactive WebAssembly Demo</p>
      </header>

      <div className="info-panel">
        <h2>About</h2>
        <p>
          This tool demonstrates the{' '}
          <a href="https://crates.io/crates/espresso-logic" target="_blank" rel="noopener noreferrer">
            <code>espresso-logic</code>
          </a>
          {' '}Rust crate, which provides Rust bindings to UC Berkeley&apos;s Espresso heuristic
          logic minimiser alongside a binary decision diagram (BDD) implementation for Boolean
          expression manipulation.
        </p>
        <p>
          <strong>How it works:</strong> The Espresso algorithm operates on a cube representation
          of Boolean functions, where each cube is a product term (conjunction of literals).
          The minimiser employs iterative heuristics—expand, reduce, and irredundant cover—to
          find a near-minimal sum-of-products form. First, Boolean expressions are converted to
          BDDs, then enumerated into cubes representing the function&apos;s ON-set (where the
          output is true). Espresso then applies its heuristics to reduce the number of cubes
          whilst maintaining logical equivalence, resulting in simplified expressions that
          require fewer gates when implemented in hardware.
        </p>
        <p>
          <strong>Syntax:</strong> Use <code>*</code> or <code>&</code> for AND,{' '}
          <code>+</code> or <code>|</code> for OR,{' '}
          <code>~</code> or <code>!</code> for NOT. Define multiple outputs as{' '}
          <code>name = expression</code> (one per line).
        </p>
        <div className="info-links">
          <a href="https://crates.io/crates/espresso-logic" target="_blank" rel="noopener noreferrer">
            📦 Crates.io
          </a>
          <a href="https://github.com/marlls1989/espresso-logic" target="_blank" rel="noopener noreferrer">
            🔧 GitHub
          </a>
          <a href="https://docs.rs/espresso-logic" target="_blank" rel="noopener noreferrer">
            📚 Documentation
          </a>
        </div>
      </div>

      <div className="workspace">
        <EditorPanel
          value={inputText}
          onChange={handleInputChange}
          onLoadExample={handleLoadExample}
          error={error}
        />

        <ResultsPanel
          result={result}
          coverType={coverType}
          onCoverTypeChange={setCoverType}
        />
      </div>

      <footer>
        <p>
          Built with{' '}
          <a href="https://react.dev" target="_blank" rel="noopener noreferrer">
            React
          </a>
          {' and '}
          <a href="https://webassembly.org" target="_blank" rel="noopener noreferrer">
            WebAssembly
          </a>
          . Original Espresso by UC Berkeley.
        </p>
      </footer>
    </div>
  );
}

export default App;

