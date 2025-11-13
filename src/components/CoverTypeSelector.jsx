export default function CoverTypeSelector({ coverType, onChange }) {
  return (
    <div className="cover-type-selector">
      <label htmlFor="cover-type">Cover Type:</label>
      <select
        id="cover-type"
        value={coverType}
        onChange={(e) => onChange(Number(e.target.value))}
        title="F: ON-set only | FD: ON-set + Don't-cares | FR: ON-set + OFF-set | FDR: Complete specification"
      >
        <option value="0">F</option>
        <option value="1">FD</option>
        <option value="2">FR</option>
        <option value="3">FDR</option>
      </select>
    </div>
  );
}

