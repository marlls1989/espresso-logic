import TruthTable from './TruthTable';

export default function ResultsPanel({ result, coverType, onCoverTypeChange }) {
  return (
    <div className="panel">
      <div className="panel-header">
        <h3>
          Optimised Results
          <span className="panel-badge" style={{ visibility: result ? 'visible' : 'hidden' }}>
            {result ? `${result.stats.numCubes} cubes` : '0 cubes'}
          </span>
        </h3>
      <div className="output-format-selector">
        <label htmlFor="cover-type">Cover Type:</label>
        <div className="tooltip-wrapper">
          <span className="help-icon" title="Cover Type Information">?</span>
          <div className="tooltip-content">
            <strong>Cover Type Selection</strong>
            <p>
              Cover types determine which sets of minterms the algorithm uses during minimisation:
            </p>
            <ul>
              <li><strong>F</strong> (ON-set only): Minimises using only the ON-set (true outputs). 
              Everything not in the ON-set is implicitly in the OFF-set.</li>
              <li><strong>FD</strong> (ON-set + Don&apos;t-cares): Allows explicit specification of 
              don&apos;t-care conditions in the input. The minimiser can assign these to either 0 or 1 
              to reduce the cover size. Only beneficial when don&apos;t-cares are actually specified.</li>
              <li><strong>FR</strong> (ON-set + OFF-set): Uses both ON-set and OFF-set during minimisation. 
              Useful when you need explicit representation of both true and false conditions.</li>
              <li><strong>FDR</strong> (Full specification): Supports all three sets (ON, OFF, and DC), 
              providing complete control when all three are explicitly specified in the input.</li>
            </ul>
            <p className="tooltip-note">
              <strong>Note:</strong> For simple Boolean expressions without explicit don&apos;t-cares, 
              F and FR typically produce similar results. FD and FDR require don&apos;t-care specifications 
              in the input to provide optimization benefits.
            </p>
          </div>
        </div>
        <select 
          id="cover-type"
          value={coverType} 
          onChange={(e) => onCoverTypeChange(Number(e.target.value))}
        >
          <option value="0">F: ON-set only</option>
          <option value="1">FD: ON-set + Don't-cares</option>
          <option value="2">FR: ON-set + OFF-set</option>
          <option value="3">FDR: Full specification</option>
        </select>
      </div>
      </div>

      <div className="empty-state" style={{ display: !result ? 'block' : 'none' }}>
        <div className="empty-state-icon">⚡</div>
        <p>Enter Boolean expressions above to see minimised results.</p>
      </div>

      <div style={{ display: result ? 'block' : 'none' }}>
        <div className="output-expressions">
          {result && result.expressions.map((expr) => (
            <div key={expr.name} className="expression-item">
              <span className="expression-name">{expr.name}</span>
              {' = '}
              {expr.expression}
            </div>
          ))}
        </div>

        <div className="stats">
          <div className="stat-item">
            <div className="stat-value">{result?.stats.numInputs || 0}</div>
            <div className="stat-label">Inputs</div>
          </div>
          <div className="stat-item">
            <div className="stat-value">{result?.stats.numOutputs || 0}</div>
            <div className="stat-label">Outputs</div>
          </div>
          <div className="stat-item">
            <div className="stat-value">{result?.stats.numCubes || 0}</div>
            <div className="stat-label">Cubes</div>
          </div>
        </div>

        <h4 style={{ marginBottom: '1rem' }}>Truth Table (Cubes)</h4>
        {result && (
          <TruthTable
            cubes={result.cubes}
            inputLabels={result.inputLabels}
            outputLabels={result.outputLabels}
          />
        )}
      </div>
    </div>
  );
}

