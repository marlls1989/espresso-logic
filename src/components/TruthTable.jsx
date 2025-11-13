export default function TruthTable({ cubes, inputLabels, outputLabels }) {
  const getCubeClass = (value) => {
    switch (value) {
      case 1:
        return 'cube-value-1';
      case 0:
        return 'cube-value-0';
      case 2:
        return 'cube-value-dc';
      default:
        return '';
    }
  };

  const getCubeChar = (value) => {
    switch (value) {
      case 1:
        return '1';
      case 0:
        return '0';
      case 2:
        return '-';
      default:
        return '?';
    }
  };

  return (
    <div className="truth-table-wrapper">
      <table>
        <thead>
          <tr>
            {inputLabels.map((label) => (
              <th key={label}>{label}</th>
            ))}
            <th style={{ borderLeft: '2px solid white' }}>→</th>
            {outputLabels.map((label) => (
              <th key={label}>{label}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {cubes.map((cube, idx) => (
            <tr key={idx}>
              {cube.inputs.map((val, i) => (
                <td key={i} className={getCubeClass(val)}>
                  {getCubeChar(val)}
                </td>
              ))}
              <td style={{ borderLeft: '2px solid #e2e8f0', background: '#f8fafc' }}>
                →
              </td>
              {cube.outputs.map((val, i) => (
                <td key={i} className={getCubeClass(val)}>
                  {getCubeChar(val)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

