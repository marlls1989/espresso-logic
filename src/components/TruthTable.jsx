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
            {outputLabels.map((label) => (
              <th key={label} style={{ borderLeft: label === outputLabels[0] ? '2px solid #cbd5e1' : undefined }}>{label}</th>
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
              {cube.outputs.map((val, i) => (
                <td key={i} className={getCubeClass(val)} style={{ borderLeft: i === 0 ? '2px solid #cbd5e1' : undefined }}>
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

