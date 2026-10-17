//! Drobne narzędzia do tekstu.
//!
//! 🦀 Nauka: iteratory po `chars()`, maszyna stanów w pętli, `&str` vs `String`.

/// Wycina tagi HTML i dekoduje kilka najczęstszych encji.
///
/// Prosta maszyna stanów – wystarcza do opisów ofert; nie jest pełnym parserem HTML.
pub fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;

    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                // Tag zastępujemy spacją, żeby "a</p><p>b" nie sklejało słów.
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }

    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Zwijamy białe znaki.
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Skraca tekst do `max` znaków (nie bajtów!), dodając „…”.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_entities() {
        let html = "<p>Rust &amp; C++</p><p>Remote&nbsp;job</p>";
        assert_eq!(strip_html(html), "Rust & C++ Remote job");
    }

    #[test]
    fn truncate_respects_unicode() {
        assert_eq!(truncate("zażółć gęślą", 6), "zażółć…");
        assert_eq!(truncate("krótki", 10), "krótki");
    }
}
