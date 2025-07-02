use gregex::{Builder, Regex};
use regex as rust_regex;

/// Compile a given pattern on all gregex engines. Return Some if it compiles
/// for all engines, or None if it fails to compile for all of them. Panics if
/// an inconcistency is detected.
pub fn compile_all(pattern: &str) -> Option<Vec<Regex>> {
    let mut engines = Vec::new();
    let mut must_fail = false;
    // Try PikeVM
    match Regex::pike_vm(pattern) {
        Ok(re) => engines.push(re),
        Err(_) => {
            must_fail = true;
        }
    }
    // Try PikeJit
    match Regex::pike_jit(pattern) {
        Ok(re) if !must_fail => engines.push(re),
        Err(_) if must_fail => (),
        _ => panic!("Inconsistency detected"),
    }

    match Builder::new(pattern).pike_jit_array() {
        Ok(re) if !must_fail => engines.push(re),
        Err(_) if must_fail => (),
        _ => panic!("Inconsistency detected"),
    }

    match Builder::new(pattern).pike_jit_cow_array() {
        Ok(re) if !must_fail => engines.push(re),
        Err(_) if must_fail => (),
        _ => panic!("Inconsistency detected"),
    }

    if must_fail { None } else { Some(engines) }
}

/// Match a pattern agains a given input on all engines,
/// including rust-regex, and compare the result of both compilation and execution.
pub fn check_all_engines(pattern: &str, input: &str) {
    // Reference engine
    let rust = rust_regex::Regex::new(pattern);
    let ours = compile_all(pattern);

    match (rust, ours) {
        (Ok(rust_re), Some(our_engines)) => {
            // find
            let rust_match = rust_re.find(input).map(|m| (m.start(), m.end()));
            for engine in &our_engines {
                let my_match = engine.find(input).map(|m| (m.span.from, m.span.to));
                assert_eq!(
                    my_match, rust_match,
                    "Mismatch for pattern {:?} input {:?} (find)",
                    pattern, input
                );
            }

            // find_all
            let rust_all: Vec<_> = rust_re
                .find_iter(input)
                .map(|m| (m.start(), m.end()))
                .collect();
            for engine in &our_engines {
                let my_all: Vec<_> = engine
                    .find_all(input)
                    .map(|m| (m.span.from, m.span.to))
                    .collect();
                assert_eq!(
                    my_all, rust_all,
                    "Mismatch for pattern {:?} input {:?} (find_all)",
                    pattern, input
                );
            }

            // find_captures
            let rust_caps = rust_re.captures(input);
            let rust_groups = rust_caps.as_ref().map(|caps| {
                (0..caps.len())
                    .map(|i| caps.get(i).map(|m| m.as_str()))
                    .collect::<Vec<_>>()
            });
            for engine in &our_engines {
                let my_caps = engine.find_captures(input);
                let my_groups = my_caps.as_ref().map(|caps| {
                    (0..caps.group_len())
                        .map(|i| caps.get(i).map(|g| g.as_str()))
                        .collect::<Vec<_>>()
                });
                assert_eq!(
                    my_groups, rust_groups,
                    "Mismatch for pattern {:?} input {:?} (find_captures)",
                    pattern, input
                );
            }

            //// find_all_captures
            let rust_all_caps: Vec<Vec<Option<&str>>> = rust_re
                .captures_iter(input)
                .map(|caps| {
                    (0..caps.len())
                        .map(|i| caps.get(i).map(|m| m.as_str()))
                        .collect()
                })
                .collect();
            for engine in &our_engines {
                let my_all_caps: Vec<Vec<Option<&str>>> = engine
                    .find_all_captures(input)
                    .map(|caps| {
                        (0..caps.group_len())
                            .map(|i| caps.get(i).map(|g| g.as_str()))
                            .collect()
                    })
                    .collect();
                assert_eq!(
                    my_all_caps, rust_all_caps,
                    "Mismatch for pattern {:?} input {:?} (find_all_captures)",
                    pattern, input
                );
            }
        }
        (Err(_), None) => {} // All failed, that's good
        (Ok(_), None) => panic!("Our engines failed to compile but rust-regex succeeded"),
        (Err(e), Some(_)) => panic!("rust-regex failed to compile but our engines succeeded: {e}"),
    }
}
