const EXAMPLES = [
  {
    name: 'XOR Function',
    description: 'Classic XOR logic',
    code: 'xor = a * ~b + ~a * b',
  },
  {
    name: 'Redundant Terms',
    description: 'Shows minimisation in action',
    code: 'out = a * b + a * b * c',
  },
  {
    name: 'XNOR (Equivalence)',
    description: 'True when inputs match',
    code: 'xnor = a * b + ~a * ~b',
  },
  {
    name: 'Majority Function',
    description: 'True if ≥2 of 3 inputs are true',
    code: 'maj = a * b + b * c + a * c',
  },
  {
    name: 'Multi-Output',
    description: 'Half adder circuit',
    code: 'sum = a * ~b + ~a * b\ncarry = a * b',
  },
  {
    name: 'Complex Expression',
    description: "De Morgan's law example",
    code: 'f = ~(a * b) + (c * ~d)',
  },
  {
    name: 'Full Adder',
    description: '3-input adder with sum and carry',
    code: 'sum = a * ~b * ~cin + ~a * b * ~cin + ~a * ~b * cin + a * b * cin\ncarry = a * b + b * cin + a * cin',
  },
  {
    name: 'Distributive Law',
    description: 'Equivalent expressions',
    code: 'f1 = a * b + a * c\nf2 = a * (b + c)',
  },
];

export default function ExamplesSelector({ onSelect }) {
  const handleChange = (e) => {
    const index = parseInt(e.target.value);
    if (index >= 0) {
      onSelect(EXAMPLES[index].code);
      // Reset to placeholder after selection
      e.target.value = '';
    }
  };

  return (
    <div className="examples-selector">
      <label htmlFor="example-select">Load Example: </label>
      <select id="example-select" onChange={handleChange} defaultValue="">
        <option value="" disabled>Choose an example...</option>
        {EXAMPLES.map((example, index) => (
          <option key={example.name} value={index}>
            {example.name} - {example.description}
          </option>
        ))}
      </select>
    </div>
  );
}

