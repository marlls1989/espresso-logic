//! Test if we can work around the bug by creating PLA from data programmatically

use espresso_logic::PLA;

#[test]
fn test_create_pla_from_cubes() {
    // Create PLA content programmatically for XOR function
    let pla_str = ".i 2\n.o 1\n.p 2\n01 1\n10 1\n.e\n";

    let pla = PLA::from_string(pla_str).expect("Failed to parse PLA");
    let before = pla.stats();
    assert_eq!(before.num_cubes_f, 2);

    let minimized = pla.minimize();
    let after = minimized.stats();

    // XOR cannot be minimized
    assert_eq!(after.num_cubes_f, 2);
}

#[test]
fn test_helper_to_create_pla_string() {
    // Test helper function that could replace CoverBuilder
    fn cubes_to_pla_string(
        num_inputs: usize,
        num_outputs: usize,
        cubes: &[(Vec<u8>, Vec<u8>)],
    ) -> String {
        let mut pla = format!(
            ".i {}\n.o {}\n.p {}\n",
            num_inputs,
            num_outputs,
            cubes.len()
        );

        for (inputs, outputs) in cubes {
            for &val in inputs {
                match val {
                    0 => pla.push('0'),
                    1 => pla.push('1'),
                    2 => pla.push('-'), // don't care
                    _ => panic!("Invalid input value"),
                }
            }
            pla.push(' ');
            for &val in outputs {
                pla.push(if val == 1 { '1' } else { '0' });
            }
            pla.push('\n');
        }
        pla.push_str(".e\n");
        pla
    }

    // Test the helper
    let cubes = vec![(vec![0, 1], vec![1]), (vec![1, 0], vec![1])];

    let pla_str = cubes_to_pla_string(2, 1, &cubes);
    let pla = PLA::from_string(&pla_str).expect("Failed to parse");

    let minimized = pla.minimize();
    assert_eq!(minimized.stats().num_cubes_f, 2);
}
