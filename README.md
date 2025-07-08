# Warning: This is very much WIP, and bugs are expected
# gregex

**gregex** is a regular expression crate for Rust, providing multiple regex
engines under a unified API. It aims to be fast, flexible, and consistent with
the popular [`regex`](https://docs.rs/regex) crate, while offering alternative
engine implementations. In particular it can do jit-compilation.

## Features
- **Multiple Engines:** Choose between a Pike VM interpreter and a JIT-compiled Pike VM engine for regex matching.
- **Compatibility:** Designed to be consistent with the [`regex`](https://docs.rs/regex) crate, with integration tests to ensure matching behavior.

## Usage

```rust
use gregex::Regex;

let re = Regex::pike_vm(r"\d+").unwrap();
assert!(re.is_match("abc123"));
let mat = re.find("abc123").unwrap();
assert_eq!(mat.as_str(), "123");

let mut count = 0;
for mat in re.find_all("12_19_11") {
   count += u32::from_str(mat.as_str()).unwrap();
}
assert!(count == 42);

// Using the JIT engine
let jit_re = Regex::pike_jit(r"foo(bar)?").unwrap();
assert!(jit_re.is_match("foobar"));
```

## Engines

- `Regex::pike_vm` — Interpreted Pike VM engine.
- `Regex::pike_jit` — JIT-compiled Pike VM engine (only available on x64).

## Testing

It includes some integration tests that compare all engines against each other
and against the `regex` crate. This ensures correctness and consistency across
implementations.

To run the tests:

```sh
cargo test
```

## Crate Organization

- `src/regex.rs`: Core API and engine dispatch
- `src/thompson/`: Engine implementations (Pike VM, JIT)
- `src/util.rs`: Shared types and helpers
- `tests/`: Integration tests

## License

Licensed under MIT or Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) or [LICENSE-APACHE](LICENSE-APACHE) for details.
