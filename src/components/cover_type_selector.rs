use espresso_logic::CoverType;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct CoverTypeSelectorProps {
    pub cover_type: CoverType,
    pub on_change: Callback<Event>,
}

#[function_component(CoverTypeSelector)]
pub fn cover_type_selector(props: &CoverTypeSelectorProps) -> Html {
    let current_value = match props.cover_type {
        CoverType::F => "F",
        CoverType::FD => "FD",
        CoverType::FR => "FR",
        CoverType::FDR => "FDR",
    };

    html! {
        <div class="controls">
            <div class="control-group">
                <label for="cover-type">{"Cover Type"}</label>
                <select
                    id="cover-type"
                    value={current_value}
                    onchange={props.on_change.clone()}
                >
                    <option value="F">{"F - ON-set only"}</option>
                    <option value="FD">{"FD - ON-set + Don't-cares"}</option>
                    <option value="FR">{"FR - ON-set + OFF-set"}</option>
                    <option value="FDR">{"FDR - ON-set + Don't-cares + OFF-set"}</option>
                </select>
                <p class="help-text">
                    {"F: Specifies where output is 1. "}
                    {"FD: Also allows don't-care conditions. "}
                    {"FR: Specifies both 1s and 0s. "}
                    {"FDR: Complete specification with don't-cares."}
                </p>
            </div>
        </div>
    }
}

