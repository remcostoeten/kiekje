use std::collections::HashMap;

/// Resolves the `:root` custom properties and `color-mix()` calls into plain values.
///
/// GTK 4.14 rejects both, so the build ships the flattened sheet and keeps the source readable.
///
/// # Examples
///
/// ```ignore
/// let css = flatten(":root { --fg: #fff; }\nlabel { color: var(--fg); }")?;
/// assert_eq!(css.trim(), "label { color: #ffffff; }");
/// ```
pub fn flatten(source: &str) -> Result<String, String> {
    let (tokens, body) = split_root(source)?;
    let mut resolved = HashMap::new();
    for name in tokens.keys() {
        resolve_token(name, &tokens, &mut resolved, &mut Vec::new())?;
    }
    let text = substitute(&body, &resolved)?;
    mix_all(&text)
}

fn split_root(source: &str) -> Result<(HashMap<String, String>, String), String> {
    let Some(start) = source.find(":root") else {
        return Ok((HashMap::new(), source.to_string()));
    };
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or(":root has no block")?;
    let close = source[open..]
        .find('}')
        .map(|offset| open + offset)
        .ok_or(":root block is not closed")?;
    let mut tokens = HashMap::new();
    for declaration in source[open + 1..close].split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let (name, value) = declaration
            .split_once(':')
            .ok_or_else(|| format!("bad :root declaration `{declaration}`"))?;
        let name = name.trim();
        if !name.starts_with("--") {
            return Err(format!("`{name}` in :root is not a custom property"));
        }
        tokens.insert(name.to_string(), value.trim().to_string());
    }
    let rest = source[close + 1..].trim_start_matches(['\n', '\r']);
    Ok((tokens, format!("{}{rest}", &source[..start])))
}

fn resolve_token(
    name: &str,
    tokens: &HashMap<String, String>,
    resolved: &mut HashMap<String, String>,
    stack: &mut Vec<String>,
) -> Result<String, String> {
    if let Some(value) = resolved.get(name) {
        return Ok(value.clone());
    }
    if stack.iter().any(|seen| seen == name) {
        return Err(format!("`{name}` refers to itself"));
    }
    let raw = tokens
        .get(name)
        .ok_or_else(|| format!("`{name}` is not defined in :root"))?;
    stack.push(name.to_string());
    let mut value = String::new();
    let mut rest = raw.as_str();
    while let Some(start) = rest.find("var(") {
        let (inner, end) = token_call(rest, start)?;
        value.push_str(&rest[..start]);
        value.push_str(&resolve_token(inner, tokens, resolved, stack)?);
        rest = &rest[end..];
    }
    value.push_str(rest);
    stack.pop();
    resolved.insert(name.to_string(), value.clone());
    Ok(value)
}

fn substitute(text: &str, resolved: &HashMap<String, String>) -> Result<String, String> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("var(") {
        let (name, end) = token_call(rest, start)?;
        let value = resolved
            .get(name)
            .ok_or_else(|| format!("`{name}` is not defined in :root"))?;
        output.push_str(&rest[..start]);
        output.push_str(value);
        rest = &rest[end..];
    }
    output.push_str(rest);
    Ok(output)
}

fn token_call(text: &str, start: usize) -> Result<(&str, usize), String> {
    let open = start + "var(".len();
    let close = text[open..]
        .find(')')
        .map(|offset| open + offset)
        .ok_or("var( is not closed")?;
    let name = text[open..close].trim();
    if !name.starts_with("--") || name.contains(',') {
        return Err(format!("unsupported var({name})"));
    }
    Ok((name, close + 1))
}

fn mix_all(text: &str) -> Result<String, String> {
    let mut text = text.to_string();
    while let Some(start) = text.rfind("color-mix(") {
        let open = start + "color-mix(".len();
        let close = text[open..]
            .find(')')
            .map(|offset| open + offset)
            .ok_or("color-mix( is not closed")?;
        let mixed = mix(&text[open..close])?;
        text.replace_range(start..close + 1, &format_color(mixed));
    }
    Ok(text)
}

type Rgba = [f64; 4];

fn mix(arguments: &str) -> Result<Rgba, String> {
    let parts: Vec<&str> = arguments.split(',').map(str::trim).collect();
    let [space, first, second] = parts[..] else {
        return Err(format!("unsupported color-mix({arguments})"));
    };
    if space != "in srgb" {
        return Err(format!("color-mix needs `in srgb`, got `{space}`"));
    }
    let (first, first_share) = stop(first)?;
    let (second, second_share) = stop(second)?;
    let (first_share, second_share) = match (first_share, second_share) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, 1.0 - a),
        (None, Some(b)) => (1.0 - b, b),
        (None, None) => (0.5, 0.5),
    };
    if (first_share + second_share - 1.0).abs() > 1e-9 {
        return Err(format!("color-mix({arguments}) does not add up to 100%"));
    }
    let alpha = first[3] * first_share + second[3] * second_share;
    let mut result = [0.0, 0.0, 0.0, alpha];
    if alpha > 0.0 {
        for channel in 0..3 {
            result[channel] = (first[channel] * first[3] * first_share
                + second[channel] * second[3] * second_share)
                / alpha;
        }
    }
    Ok(result)
}

fn stop(text: &str) -> Result<(Rgba, Option<f64>), String> {
    let (color, share) = match text.rsplit_once(' ') {
        Some((color, share)) if share.ends_with('%') => {
            let percent: f64 = share
                .trim_end_matches('%')
                .parse()
                .map_err(|_| format!("bad percentage `{share}`"))?;
            (color.trim(), Some(percent / 100.0))
        }
        _ => (text, None),
    };
    Ok((parse_color(color)?, share))
}

fn parse_color(text: &str) -> Result<Rgba, String> {
    if text == "transparent" {
        return Ok([0.0, 0.0, 0.0, 0.0]);
    }
    if let Some(hex) = text.strip_prefix('#') {
        let digits: Vec<f64> = hex
            .chars()
            .map(|digit| digit.to_digit(16).map(f64::from))
            .collect::<Option<_>>()
            .ok_or_else(|| format!("bad colour `{text}`"))?;
        return match digits[..] {
            [r, g, b] => Ok([r * 17.0, g * 17.0, b * 17.0, 1.0]),
            [r1, r2, g1, g2, b1, b2] => Ok([r1 * 16.0 + r2, g1 * 16.0 + g2, b1 * 16.0 + b2, 1.0]),
            _ => Err(format!("bad colour `{text}`")),
        };
    }
    if let Some(inner) = text
        .strip_prefix("rgba(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let values: Vec<f64> = inner
            .split(',')
            .map(|value| value.trim().parse().ok())
            .collect::<Option<_>>()
            .ok_or_else(|| format!("bad colour `{text}`"))?;
        if let [r, g, b, a] = values[..] {
            return Ok([r, g, b, a]);
        }
    }
    Err(format!("unsupported colour `{text}` in color-mix"))
}

fn format_color([r, g, b, a]: Rgba) -> String {
    let channel = |value: f64| value.round().clamp(0.0, 255.0) as u8;
    if (a - 1.0).abs() < 1e-9 {
        return format!("#{:02x}{:02x}{:02x}", channel(r), channel(g), channel(b));
    }
    let alpha = format!("{a:.3}");
    let alpha = alpha.trim_end_matches('0').trim_end_matches('.');
    format!(
        "rgba({}, {}, {}, {alpha})",
        channel(r),
        channel(g),
        channel(b)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_two_opaque_colours_by_share() {
        let css = flatten("a { color: color-mix(in srgb, #ffffff 25%, #000000); }").unwrap();
        assert_eq!(css, "a { color: #404040; }");
    }

    #[test]
    fn mixes_with_transparent_into_alpha() {
        let css = flatten("a { color: color-mix(in srgb, #fe5101 14%, transparent); }").unwrap();
        assert_eq!(css, "a { color: rgba(254, 81, 1, 0.14); }");
    }

    #[test]
    fn resolves_tokens_that_use_other_tokens() {
        let source = ":root {\n  --bg: #000;\n  --fg: #fff;\n  --surface: color-mix(in srgb, var(--fg) 50%, var(--bg));\n}\n\nbox { background-color: color-mix(in srgb, var(--surface) 50%, transparent); }\n";
        let css = flatten(source).unwrap();
        assert_eq!(css, "box { background-color: rgba(128, 128, 128, 0.5); }\n");
    }

    #[test]
    fn keeps_non_colour_tokens_verbatim() {
        let source = ":root { --radius: 8px; --specular: inset 0 1px color-mix(in srgb, #ffffff 6%, transparent); }\nb { border-radius: var(--radius); box-shadow: var(--specular); }";
        let css = flatten(source).unwrap();
        assert_eq!(
            css,
            "b { border-radius: 8px; box-shadow: inset 0 1px rgba(255, 255, 255, 0.06); }"
        );
    }

    #[test]
    fn rejects_undefined_tokens() {
        assert!(flatten("a { color: var(--missing); }").is_err());
    }

    #[test]
    fn rejects_other_colour_spaces() {
        assert!(flatten("a { color: color-mix(in oklab, #fff 50%, #000); }").is_err());
    }

    #[test]
    fn flattens_the_app_stylesheet_completely() {
        let css = flatten(include_str!("../resources/style.css")).unwrap();
        assert!(!css.contains("var("));
        assert!(!css.contains("color-mix("));
        assert!(!css.contains(":root"));
        assert!(!css.contains("--"));
    }
}
