use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};
use gloo_console::log;
use web_sys::{HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

mod components;
use components::{CoverTypeSelector, ExamplesSelector, TruthTable};

#[derive(Clone, PartialEq)]
struct ProcessedResult {
    expressions: Vec<(String, String)>,
    cover: Cover,
    stats: Stats,
}

#[derive(Clone, PartialEq)]
struct Stats {
    num_inputs: usize,
    num_outputs: usize,
    num_cubes: usize,
}

#[function_component(App)]
fn app() -> Html {
    let input_text = use_state(|| String::from("x = a * b + a * b * c\ny = a + b"));
    let cover_type = use_state(|| CoverType::F);
    let result = use_state(|| Option::<ProcessedResult>::None);
    let error = use_state(|| Option::<String>::None);

    let on_input_change = {
        let input_text = input_text.clone();
        Callback::from(move |e: Event| {
            let target: HtmlTextAreaElement = e.target_unchecked_into();
            input_text.set(target.value());
        })
    };

    let on_cover_type_change = {
        let cover_type = cover_type.clone();
        Callback::from(move |e: Event| {
            let target: HtmlSelectElement = e.target_unchecked_into();
            let new_type = match target.value().as_str() {
                "FD" => CoverType::FD,
                "FR" => CoverType::FR,
                "FDR" => CoverType::FDR,
                _ => CoverType::F,
            };
            cover_type.set(new_type);
        })
    };

    let on_minimize = {
        let input_text = input_text.clone();
        let cover_type = cover_type.clone();
        let result = result.clone();
        let error = error.clone();
        Callback::from(move |_| {
            error.set(None);
            result.set(None);

            match process_expressions(&input_text, *cover_type) {
                Ok(processed) => {
                    result.set(Some(processed));
                }
                Err(e) => {
                    error.set(Some(e));
                }
            }
        })
    };

    let on_example_select = {
        let input_text = input_text.clone();
        Callback::from(move |example: String| {
            input_text.set(example);
        })
    };

    html! {
        <div class="app-container">
            <header>
                <h1>{"Espresso Logic Minimiser"}</h1>
                <p>{"Interactive WebAssembly Demo"}</p>
            </header>

            <div class="info-panel">
                <h2>{"About"}</h2>
                <p>
                    {"This is an interactive demonstration of the "}
                    <strong>{"Espresso heuristic logic minimiser"}</strong>
                    {" from UC Berkeley. Enter Boolean expressions and see them minimised in real-time."}
                </p>
                <p>
                    {"Syntax: Use "} <code>{"*"}</code> {" or "} <code>{"&"}</code> {" for AND, "}
                    <code>{"+"}</code> {" or "} <code>{"|"}</code> {" for OR, "}
                    <code>{"~"}</code> {" or "} <code>{"!"}</code> {" for NOT. "}
                    {"Define multiple outputs as "} <code>{"name = expression"}</code> {" (one per line)."}
                </p>
                <div class="info-links">
                    <a href="https://crates.io/crates/espresso-logic" target="_blank">{"📦 Crates.io"}</a>
                    <a href="https://github.com/marlls1989/espresso-logic" target="_blank">{"🔧 GitHub"}</a>
                    <a href="https://docs.rs/espresso-logic" target="_blank">{"📚 Documentation"}</a>
                </div>
            </div>

            <CoverTypeSelector cover_type={*cover_type} on_change={on_cover_type_change} />

            <ExamplesSelector on_select={on_example_select} />

            <div class="workspace">
                <div class="panel">
                    <h3>{"Input Expressions"}</h3>
                    <div class="editor-wrapper">
                        <textarea
                            value={(*input_text).clone()}
                            onchange={on_input_change}
                            placeholder="x = a * b + a * b * c\ny = a + b"
                        />
                    </div>
                    <button onclick={on_minimize}>{"Minimise"}</button>

                    if let Some(err) = (*error).clone() {
                        <div class="error-message">
                            <strong>{"Error: "}</strong> {err}
                        </div>
                    }
                </div>

                <div class="panel">
                    <h3>
                        {"Optimised Results"}
                        if let Some(ref res) = *result {
                            <span class="panel-badge">{format!("{} cubes", res.stats.num_cubes)}</span>
                        }
                    </h3>

                    if let Some(ref res) = *result {
                        <div class="output-expressions">
                            { for res.expressions.iter().map(|(name, expr)| {
                                html! {
                                    <div class="expression-item">
                                        <span class="expression-name">{name}</span>
                                        {" = "}
                                        {expr}
                                    </div>
                                }
                            })}
                        </div>

                        <div class="stats">
                            <div class="stat-item">
                                <div class="stat-value">{res.stats.num_inputs}</div>
                                <div class="stat-label">{"Inputs"}</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-value">{res.stats.num_outputs}</div>
                                <div class="stat-label">{"Outputs"}</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-value">{res.stats.num_cubes}</div>
                                <div class="stat-label">{"Cubes"}</div>
                            </div>
                        </div>

                        <h4 style="margin-bottom: 1rem;">{"Truth Table (Cubes)"}</h4>
                        <TruthTable cover={res.cover.clone()} />
                    } else {
                        <div class="empty-state">
                            <div class="empty-state-icon">{"⚡"}</div>
                            <p>{"Enter Boolean expressions above and click Minimise to see results."}</p>
                        </div>
                    }
                </div>
            </div>

            <footer>
                <p>
                    {"Built with "}
                    <a href="https://yew.rs" target="_blank">{"Yew"}</a>
                    {" and "}
                    <a href="https://webassembly.org" target="_blank">{"WebAssembly"}</a>
                    {". Original Espresso by UC Berkeley."}
                </p>
            </footer>
        </div>
    }
}

fn process_expressions(input: &str, cover_type: CoverType) -> Result<ProcessedResult, String> {
    let mut cover = Cover::new(cover_type);

    // Parse input: each line should be "name = expression"
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid line format: '{}'. Expected 'name = expression'", line));
        }

        let name = parts[0].trim();
        let expr_str = parts[1].trim();

        if name.is_empty() {
            return Err(format!("Empty output name in line: '{}'", line));
        }

        let expr = BoolExpr::parse(expr_str)
            .map_err(|e| format!("Parse error in '{}': {}", expr_str, e))?;

        cover
            .add_expr(&expr, name)
            .map_err(|e| format!("Error adding expression '{}': {}", name, e))?;
    }

    if cover.num_outputs() == 0 {
        return Err("No valid expressions found. Please enter at least one expression.".to_string());
    }

    log!("Before minimisation:", cover.num_cubes(), "cubes");

    // Minimise the cover
    let minimised = cover
        .minimize()
        .map_err(|e| format!("Minimisation error: {}", e))?;

    log!("After minimisation:", minimised.num_cubes(), "cubes");

    // Extract optimised expressions
    let mut expressions = Vec::new();
    for i in 0..minimised.num_outputs() {
        let label = minimised.output_labels().get(i)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("y{}", i));
        
        let expr = minimised.to_expr(&label)
            .map_err(|e| format!("Error converting output '{}': {}", label, e))?;
        
        expressions.push((label, expr.to_string()));
    }

    let stats = Stats {
        num_inputs: minimised.num_inputs(),
        num_outputs: minimised.num_outputs(),
        num_cubes: minimised.num_cubes(),
    };

    Ok(ProcessedResult {
        expressions,
        cover: minimised,
        stats,
    })
}

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}
