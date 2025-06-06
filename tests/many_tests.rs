mod utils;

#[test]
fn test_many() {
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
        ),
        (r"^$", ""),
        (r"^[a-z]+@[a-z]+\.com$", "foo@bar.com foo@baz.com"),
        (r"\s+", "a b\tc\nd"),
        (r"(?m)^foo", "foobar\nfoo\nbarfoo"),
        (r"bar$", "foobar\nfoo\nbarfoo"),
        (r"colou?r", "color colour colouur"),
        (r"ab{2,4}c", "abc abbc abbbc abbbbc abbbbbc"),
        (r"(?:abc)+", "abcabcabcx"),
        (r"(?i)abc", "ABC abc AbC"),
        (r"[A-Z]{2,}", "abc DEF GHI jkl"),
        (r"[^0-9]+", "abc123!@#"),
        (r"^foo", "foo\nbar\nfoo\nbaz"),
        (r"(?m)^foo$", "foo\nbar\nfoo\nbaz"),
        (r"(?mR)^foo$", "foo\r\nbar\r\nfoo\nbaz\nfoo"),
        (r".*[^A-Z]|[A-Z]", "AAAAAAAAAAAAAAAAAAAA"),
        (r".*[^A-Z]|[A-Z]", "AAAAB"),
        (r".*[^A-Z]|[A-Z]", "AABAB"),
        (
            r"(\s*)((?:# [Nn][Oo][Qq][Aa])(?::\s?(([A-Z]+[0-9]+(?:[,\s]+)?)+))?)",
            "                     # noqua:A123A                                                                                       # noqa
                                    # noqa
                                    # noqa            # noqua:A123A
                                    # noqa
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A
                                    # noqa            # noqua:A123A




















































































































































                                     # noqa            # noqua:A123A
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
                                    # noqa
            ",
        ), //(r"\bword\b", "word sword words word."),
           // (r"(?s)a.*b", "a\n\nb"),
           // (r"(\d{2,4})-(\d{2})-(\d{2,4})", "2023-06-01 99-12-9999"),
           // (r"([a-z])\1", "bookkeeper"),
           // (
           //     r"(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)",
           //     "Valid IP: 192.168.1.1 Invalid IP: 999.999.999.999",
           // ),
           // (r"\Astart", "start of line\nnot at start"),
           // (r"end\z", "not at end\nthis is the end"),
           // (r"(?<=foo)bar", "foobar foo bar foobarbar"),
           // (r"(?<!foo)bar", "bar foobar foo barfoobar"),
           // (r"([A-Za-z]+)\s+\1", "hello hello world world test test"),
           // (r"\p{L}+", "Русский English 中文 عربى"),
           // (r"\d{3,}", "12 123 1234 12345"),
           // (r"([a-z]+)(?=\d)", "abc123 def456 ghi"),
           // (r"([a-z]+)(?!\d)", "abc123 def456 ghi"),
           // (r"(?P<name>[A-Z][a-z]+)", "Alice Bob carol dave"),
           // (r"(?P<num>\d+)", "There are 15 apples and 42 oranges."),
           // (r"(?P<word>\w+)", "hello"),
           // (
           //     r"(?P<htmltag><([a-z]+)[^>]*>)",
           //     "<div class=\"main\"><span>Text</span></div>",
           // ),
           // (r"(?P<date>\d{4}-\d{2}-\d{2})", "Today is 2024-06-01."),
           // (r"(?P<time>\d{2}:\d{2}:\d{2})", "The time is 12:34:56."),
           // (
           //     r"(?P<hex>#(?:[0-9a-fA-F]{3}){1,2})",
           //     "Colors: #fff #123456 #abc",
           // ),
           // (
           //     r"(?P<url>https?://[^\s]+)",
           //     "Visit https://example.com or http://test.org.",
           // ),
           // (
           //     r"(?P<email>[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,})",
           //     "Contact: foo@bar.com, test@example.org",
           // ),
    ];
    for (pattern, input) in CASES {
        println!("Testing: {pattern} on {input}");
        utils::check_all_engines(pattern, input);
    }
}
