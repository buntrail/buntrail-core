fn main() {
    let text = buntrail_core::Text::from("hello 🐇");
    assert_eq!(text.to_string(), "hello 🐇");
}
