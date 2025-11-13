import React, { useRef, useEffect } from 'react';

export default function EditorPanel({ value, onChange, onLoadExample, error }) {
  const textareaRef = useRef(null);
  
  const lines = value.split('\n');

  // Debug: log when error changes
  useEffect(() => {
    if (error) {
      console.log('Error detected in EditorPanel:', error);
    }
  }, [error]);

  // Render line with error highlighting
  const renderLineWithError = (lineText, lineIndex) => {
    if (!error || error.line !== lineIndex) {
      return lineText || ' ';
    }

    // Support both old format (startCol/endCol) and new format (errorRegions)
    const regions = error.errorRegions || [{ startCol: error.startCol || 0, endCol: error.endCol || lineText.length }];
    
    // Sort regions by start position
    const sortedRegions = [...regions].sort((a, b) => a.startCol - b.startCol);

    // Build an array of segments: [text, isError, text, isError, ...]
    const segments = [];
    let currentPos = 0;

    for (const region of sortedRegions) {
      const start = region.startCol;
      const end = region.endCol;

      // Add normal text before this error region
      if (start > currentPos) {
        segments.push({ text: lineText.substring(currentPos, start), isError: false });
      }

      // Add error region
      const errorText = lineText.substring(start, end);
      const displayText = errorText.length > 0 ? errorText : '\u00A0';
      segments.push({ text: displayText, isError: true });

      currentPos = end;
    }

    // Add any remaining text after the last error
    if (currentPos < lineText.length) {
      segments.push({ text: lineText.substring(currentPos), isError: false });
    }

    // If no segments (empty line), return space
    if (segments.length === 0) {
      return ' ';
    }

    return (
      <>
        {segments.map((segment, idx) =>
          segment.isError ? (
            <span key={idx} className="error-token">{segment.text}</span>
          ) : (
            <React.Fragment key={idx}>{segment.text}</React.Fragment>
          )
        )}
      </>
    );
  };

  return (
    <div className="panel">
      <div className="panel-header">
        <h3>Input Expressions</h3>
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
          <button 
            className="load-example-btn"
            onClick={(e) => {
              const examples = [
                'xor = a * ~b + ~a * b',
                'out = a * b + a * b * c',
                'xnor = a * b + ~a * ~b',
                'maj = a * b + b * c + a * c',
                'sum = a * ~b + ~a * b\ncarry = a * b',
                'f = ~(a * b) + (c * ~d)',
                'sum = a * ~b * ~cin + ~a * b * ~cin + ~a * ~b * cin + a * b * cin\ncarry = a * b + b * cin + a * cin',
                'f1 = a * b + a * c\nf2 = a * (b + c)',
              ];
              const exampleNames = [
                'XOR', 'Redundant', 'XNOR', 'Majority', 
                'Half Adder', 'Complex', 'Full Adder', 'Distributive'
              ];
              const index = parseInt(prompt(`Choose example:\n${exampleNames.map((n, i) => `${i}: ${n}`).join('\n')}`));
              if (index >= 0 && index < examples.length) {
                onLoadExample(examples[index]);
              }
            }}
          >
            📋 Load Example
          </button>
        </div>
      </div>
      <div className="editor-wrapper">
        <div className="editor-with-overlay">
          <div 
            className="line-highlights" 
            aria-hidden="true" 
            style={{ visibility: error && error.line >= 0 ? 'visible' : 'hidden' }}
          >
            {lines.map((line, idx) => (
              <div
                key={idx}
                className={error && error.line === idx ? 'line-highlight error-line' : 'line-highlight'}
              >
                {renderLineWithError(line, idx)}
              </div>
            ))}
          </div>
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder="x = a * b + a * b * c&#10;y = a + b"
            className={error && error.line >= 0 ? 'has-error' : ''}
          />
        </div>
      </div>
    </div>
  );
}

