use espresso_logic::Cover;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct TruthTableProps {
    pub cover: Cover,
}

#[function_component(TruthTable)]
pub fn truth_table(props: &TruthTableProps) -> Html {
    let cover = &props.cover;

    let input_labels = cover.input_labels();
    let output_labels = cover.output_labels();

    html! {
        <div class="truth-table-wrapper">
            <table>
                <thead>
                    <tr>
                        { for input_labels.iter().map(|label| html! {
                            <th>{label.as_ref()}</th>
                        })}
                        <th style="border-left: 2px solid white;">{"→"}</th>
                        { for output_labels.iter().map(|label| html! {
                            <th>{label.as_ref()}</th>
                        })}
                    </tr>
                </thead>
                <tbody>
                    { for cover.cubes_iter().map(|(inputs, outputs)| {
                        html! {
                            <tr>
                                { for inputs.iter().map(|val| {
                                    let (text, class) = match val {
                                        Some(true) => ("1", "cube-value-1"),
                                        Some(false) => ("0", "cube-value-0"),
                                        None => ("-", "cube-value-dc"),
                                    };
                                    html! { <td class={class}>{text}</td> }
                                })}
                                <td style="border-left: 2px solid #e2e8f0; background: #f8fafc;">{"→"}</td>
                                { for outputs.iter().map(|val| {
                                    let (text, class) = match val {
                                        Some(true) => ("1", "cube-value-1"),
                                        Some(false) => ("0", "cube-value-0"),
                                        None => ("-", "cube-value-dc"),
                                    };
                                    html! { <td class={class}>{text}</td> }
                                })}
                            </tr>
                        }
                    })}
                </tbody>
            </table>
        </div>
    }
}

