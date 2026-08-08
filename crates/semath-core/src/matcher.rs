pub(crate) struct PrimitiveMatcherSpec {
    pub expression: &'static str,
    pub parameter_captures: &'static [&'static [usize]],
}

const ONE_CAPTURE: &[&[usize]] = &[&[1]];
const TWO_CAPTURES: &[&[usize]] = &[&[1], &[2]];
const QUADRATIC_CAPTURES: &[&[usize]] = &[&[1, 3], &[2]];

pub(crate) fn primitive_matcher(name: &str) -> Option<PrimitiveMatcherSpec> {
    let spec = match name {
        "binary-product" => PrimitiveMatcherSpec {
            expression: r"^\s*([A-Za-z])\s*(?:\\cdot\s*)?([A-Za-z])\s*$",
            parameter_captures: TWO_CAPTURES,
        },
        "conditional-probability" => PrimitiveMatcherSpec {
            expression: r"^\s*(?:\\mathbb\s*\{\s*P\s*\}|\\mathrm\s*\{\s*P\s*\}|\\Pr)\s*(?:\\left\s*)?\(\s*([A-Za-z])\s*(?:\\mid|\\vert|\|)\s*([A-Za-z])\s*(?:\\right\s*)?\)\s*$",
            parameter_captures: TWO_CAPTURES,
        },
        "event-probability" => PrimitiveMatcherSpec {
            expression: r"^\s*(?:\\mathbb\s*\{\s*P\s*\}|\\mathrm\s*\{\s*P\s*\}|\\Pr)\s*(?:\\left\s*)?\(\s*([A-Za-z])\s*(?:\\right\s*)?\)\s*$",
            parameter_captures: ONE_CAPTURE,
        },
        "expectation" => PrimitiveMatcherSpec {
            expression: r"^\s*(?:\\mathbb\s*\{\s*E\s*\}|\\mathrm\s*\{\s*E\s*\})\s*(?:\\left\s*)?\[\s*([A-Za-z])\s*(?:\\right\s*)?\]\s*$",
            parameter_captures: ONE_CAPTURE,
        },
        "quadratic-form" => PrimitiveMatcherSpec {
            expression: r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*([A-Za-z])\s*([A-Za-z])\s*$",
            parameter_captures: QUADRATIC_CAPTURES,
        },
        "transpose" => PrimitiveMatcherSpec {
            expression: r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*$",
            parameter_captures: ONE_CAPTURE,
        },
        "transposed-binary-product" => PrimitiveMatcherSpec {
            expression: r"^\s*([A-Za-z])\s*\^\s*(?:\{\s*\\top\s*\}|\\top)\s*([A-Za-z])\s*$",
            parameter_captures: TWO_CAPTURES,
        },
        "variance" => PrimitiveMatcherSpec {
            expression: r"^\s*\\(?:operatorname|mathrm)\s*\{\s*Var\s*\}\s*(?:\\left\s*)?\(\s*([A-Za-z])\s*(?:\\right\s*)?\)\s*$",
            parameter_captures: ONE_CAPTURE,
        },
        _ => return None,
    };
    Some(spec)
}

#[cfg(test)]
mod tests {
    use super::primitive_matcher;
    use regex::Regex;

    #[test]
    fn primitive_capture_plans_match_their_regexes() {
        for name in [
            "binary-product",
            "conditional-probability",
            "event-probability",
            "expectation",
            "quadratic-form",
            "transpose",
            "transposed-binary-product",
            "variance",
        ] {
            let spec = primitive_matcher(name).expect("known primitive");
            let regex = Regex::new(spec.expression).expect("bounded primitive regex");
            let largest_capture = spec
                .parameter_captures
                .iter()
                .flat_map(|captures| captures.iter())
                .copied()
                .max()
                .unwrap_or(0);
            assert!(largest_capture < regex.captures_len(), "{name}");
            assert!(
                spec.parameter_captures
                    .iter()
                    .all(|captures| !captures.is_empty()),
                "{name}"
            );
        }
    }
}
