use super::types::EmailError;
use std::collections::HashMap;

/// Render a template string by replacing {{key}} placeholders
/// with values from the provided map.
///
/// - Delimiters: `{{` and `}}`
/// - Unknown placeholders are left as-is (so you can catch them in tests)
/// - Keys are case-sensitive
pub fn render(template: &str, vars: &HashMap<&str, &str>) -> String {
    let mut output = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key); // {{key}}
        output = output.replace(&placeholder, value);
    }
    output
}

/// Convenience macro — build the vars HashMap inline
#[macro_export]
macro_rules! vars {
    ($($key:expr => $val:expr),* $(,)?) => {{
        let mut m = std::collections::HashMap::new();
        $(m.insert($key, $val);)*
        m
    }};
}

/// Assert that no {{placeholder}} remains after rendering.
/// Use in tests to catch missing variables.
pub fn assert_no_placeholders(rendered: &str) {
    if rendered.contains("{{") {
        let remaining: Vec<&str> = rendered
            .split("{{")
            .skip(1)
            .filter_map(|s| s.split("}}").next())
            .collect();
        panic!(
            "Unresolved placeholders in rendered template: {:?}",
            remaining
        );
    }
}
