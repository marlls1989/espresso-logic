use espresso_logic::{BoolExpr, Cover, CoverType, Minimizable};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Process Boolean expressions and return minimised results as JSON
///
/// # Safety
///
/// This function is unsafe because it dereferences a raw pointer.
/// The caller must ensure that `input_ptr` is a valid, non-null pointer to a C string.
#[no_mangle]
pub unsafe extern "C" fn minimise_expressions(
    input_ptr: *const c_char,
    cover_type: u32,
) -> *mut c_char {
    let input = {
        if input_ptr.is_null() {
            return create_error("Null input pointer");
        }
        match CStr::from_ptr(input_ptr).to_str() {
            Ok(s) => s,
            Err(_) => return create_error("Invalid UTF-8 in input"),
        }
    };

    let cover_type = match cover_type {
        0 => CoverType::F,
        1 => CoverType::FD,
        2 => CoverType::FR,
        3 => CoverType::FDR,
        _ => return create_error("Invalid cover type"),
    };

    match process_expressions(input, cover_type) {
        Ok(json) => match CString::new(json) {
            Ok(c_str) => c_str.into_raw(),
            Err(_) => create_error("Failed to create output string"),
        },
        Err(e) => create_error(&e),
    }
}

/// Free a string allocated by Rust
///
/// # Safety
///
/// This function is unsafe because it takes ownership of a raw pointer.
/// The caller must ensure that `ptr` was previously allocated by Rust and hasn't been freed yet.
#[no_mangle]
pub unsafe extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

fn create_error(msg: &str) -> *mut c_char {
    let escaped = escape_json_string(msg);
    let error_json = format!(r#"{{"error":"{}"}}"#, escaped);
    CString::new(error_json).unwrap().into_raw()
}

fn escape_json_string(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            c if c.is_control() => format!("\\u{:04x}", c as u32),
            c => c.to_string(),
        })
        .collect()
}

fn process_expressions(input: &str, cover_type: CoverType) -> Result<String, String> {
    let mut cover = Cover::new(cover_type);

    for (line_num, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Line {}: Invalid format '{}' - expected: name = expression",
                line_num + 1,
                line
            ));
        }

        let name = parts[0].trim();
        let expr_str = parts[1].trim();

        if name.is_empty() {
            return Err(format!(
                "Line {}: Missing output name in '{}'",
                line_num + 1,
                line
            ));
        }

        let expr = BoolExpr::parse(expr_str).map_err(|e| {
            // Clean up error message
            let err_msg = format!("{}", e);
            format!(
                "Line {}: {} in expression '{}'",
                line_num + 1,
                err_msg,
                expr_str
            )
        })?;

        cover.add_expr(&expr, name).map_err(|e| {
            format!(
                "Line {}: Error adding expression '{}' - {}",
                line_num + 1,
                name,
                e
            )
        })?;
    }

    if cover.num_outputs() == 0 {
        return Err("No valid expressions found".to_string());
    }

    let minimised = cover
        .minimize()
        .map_err(|e| format!("Minimisation failed: {}", e))?;

    let mut expressions = Vec::new();
    for i in 0..minimised.num_outputs() {
        let label = minimised
            .output_labels()
            .get(i)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("y{}", i));

        let expr = minimised
            .to_expr(&label)
            .map_err(|e| format!("Error converting output '{}': {}", label, e))?;

        expressions.push((label, expr.to_string()));
    }

    let mut cubes = Vec::new();
    for (inputs, outputs) in minimised.cubes_iter() {
        cubes.push((
            inputs
                .iter()
                .map(|v| match v {
                    Some(true) => 1,
                    Some(false) => 0,
                    None => 2,
                })
                .collect::<Vec<_>>(),
            outputs
                .iter()
                .map(|v| match v {
                    Some(true) => 1,
                    Some(false) => 0,
                    None => 2,
                })
                .collect::<Vec<_>>(),
        ));
    }

    let input_labels: Vec<&str> = minimised
        .input_labels()
        .iter()
        .map(|s| s.as_ref())
        .collect();
    let output_labels: Vec<&str> = minimised
        .output_labels()
        .iter()
        .map(|s| s.as_ref())
        .collect();

    let json = format!(
        r#"{{"expressions":{},"cubes":{},"inputLabels":{},"outputLabels":{},"stats":{{"numInputs":{},"numOutputs":{},"numCubes":{}}}}}"#,
        serde_json(&expressions),
        serde_json_cubes(&cubes),
        serde_json_labels(&input_labels),
        serde_json_labels(&output_labels),
        minimised.num_inputs(),
        minimised.num_outputs(),
        minimised.num_cubes(),
    );

    Ok(json)
}

fn serde_json(expressions: &[(String, String)]) -> String {
    let items: Vec<String> = expressions
        .iter()
        .map(|(name, expr)| {
            format!(
                r#"{{"name":"{}","expression":"{}"}}"#,
                escape_json(name),
                escape_json(expr)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn serde_json_cubes(cubes: &[(Vec<i32>, Vec<i32>)]) -> String {
    let items: Vec<String> = cubes
        .iter()
        .map(|(inputs, outputs)| {
            let inputs_str = inputs
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let outputs_str = outputs
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!(r#"{{"inputs":[{inputs_str}],"outputs":[{outputs_str}]}}"#)
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn serde_json_labels(labels: &[&str]) -> String {
    let items: Vec<String> = labels
        .iter()
        .map(|s| format!(r#""{}""#, escape_json(s)))
        .collect();
    format!("[{}]", items.join(","))
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn main() {
    println!("Espresso Logic Minimiser WASM module loaded");
}
