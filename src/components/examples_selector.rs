use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ExamplesSelectorProps {
    pub on_select: Callback<String>,
}

struct Example {
    name: &'static str,
    description: &'static str,
    code: &'static str,
}

const EXAMPLES: &[Example] = &[
    Example {
        name: "XOR Function",
        description: "Classic XOR logic",
        code: "xor = a * ~b + ~a * b",
    },
    Example {
        name: "Redundant Terms",
        description: "Shows minimisation in action",
        code: "out = a * b + a * b * c",
    },
    Example {
        name: "XNOR (Equivalence)",
        description: "True when inputs match",
        code: "xnor = a * b + ~a * ~b",
    },
    Example {
        name: "Majority Function",
        description: "True if ≥2 of 3 inputs are true",
        code: "maj = a * b + b * c + a * c",
    },
    Example {
        name: "Multi-Output",
        description: "Half adder circuit",
        code: "sum = a * ~b + ~a * b\ncarry = a * b",
    },
    Example {
        name: "Complex Expression",
        description: "De Morgan's law example",
        code: "f = ~(a * b) + (c * ~d)",
    },
    Example {
        name: "Full Adder",
        description: "3-input adder with sum and carry",
        code: "sum = a * ~b * ~cin + ~a * b * ~cin + ~a * ~b * cin + a * b * cin\ncarry = a * b + b * cin + a * cin",
    },
    Example {
        name: "Distributive Law",
        description: "Equivalent expressions",
        code: "f1 = a * b + a * c\nf2 = a * (b + c)",
    },
];

#[function_component(ExamplesSelector)]
pub fn examples_selector(props: &ExamplesSelectorProps) -> Html {
    html! {
        <div class="examples-selector">
            <h3>{"Example Expressions"}</h3>
            <div class="examples-grid">
                { for EXAMPLES.iter().map(|example| {
                    let code = example.code.to_string();
                    let on_click = {
                        let on_select = props.on_select.clone();
                        Callback::from(move |_| {
                            on_select.emit(code.clone());
                        })
                    };

                    html! {
                        <button class="example-btn" onclick={on_click}>
                            <strong>{example.name}</strong>
                            <div style="font-size: 0.75rem; color: #64748b;">
                                {example.description}
                            </div>
                        </button>
                    }
                })}
            </div>
        </div>
    }
}

