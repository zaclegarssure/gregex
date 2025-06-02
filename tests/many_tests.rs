mod utils;

const CASES: &[(&str, &str)] = &[
    (r"\d+", "abc123def"),
    (r"foo", "foobar"),
    (r"bar", "foobar"),
    (r"baz", "foobar"),
    (r"(\w+)-(\d+)", "test-42"),
    (r"(\d+)?", ""),
    (r"[a-z]{3}", "xyz"),
    (r"invalid[", "anything"),
    (r"\d+=\d+", "124221=12323=2=abd"),
    (
        r"Sherlock Holmes|Shrelock Holm|John Watson|Irene Adler|Inspector Lestrade|Professor Moriarty",
        "Professor Moriarty
        Sherlock Holmes
        John Watson
        Irene Adler
        ",
    ),
    (
        r".*d",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaad",
    ), //(r"^$", ""),
       //(r"(?P<word>\w+)", "hello"),
];

#[test]
fn test_many() {
    for (pattern, input) in CASES {
        utils::check_all_engines(pattern, input);
    }
}
