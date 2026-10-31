//! ISBNs as Kernsetzer reads them, one rule for print pre-flight, the EPUB's
//! `dc:identifier` and the app's book details.

/// The digits of a valid ISBN-10 or ISBN-13, an ISBN-10's check digit `X`
/// in capitals: hyphens, spaces and an `ISBN` prefix allowed, check digit
/// verified. `None` for anything else.
pub fn digits(s: &str) -> Option<String> {
    let s = s.trim();
    let s = match s.get(..4) {
        Some(p) if p.eq_ignore_ascii_case("isbn") => &s[4..],
        _ => s,
    };
    let s = s.trim_start_matches([':', ' ']);
    let mut digits = String::new();
    for c in s.chars() {
        match c {
            '0'..='9' => digits.push(c),
            'X' | 'x' => digits.push('X'),
            '-' | ' ' => {}
            _ => return None,
        }
    }
    let value = |c: char| c.to_digit(10);
    let ok = match digits.len() {
        13 => {
            let mut sum = 0;
            for (i, c) in digits.chars().enumerate() {
                sum += value(c)? * if i % 2 == 0 { 1 } else { 3 };
            }
            sum % 10 == 0
        }
        10 => {
            let mut sum = 0;
            for (i, c) in digits.chars().enumerate() {
                // `X` (10) only as the check digit.
                let v = match c {
                    'X' if i == 9 => 10,
                    c => value(c)?,
                };
                sum += v * (10 - i as u32);
            }
            sum % 11 == 0
        }
        _ => false,
    };
    ok.then_some(digits)
}

/// Whether `s` is a valid ISBN-10 or ISBN-13 as [`digits`] reads one.
pub fn is_valid(s: &str) -> bool {
    digits(s).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_of_valid_isbns() {
        assert_eq!(
            digits("978-3-16-148410-0").as_deref(),
            Some("9783161484100")
        );
        assert_eq!(
            digits("ISBN: 978 3 16 148410 0").as_deref(),
            Some("9783161484100")
        );
        assert_eq!(digits("isbn 0-306-40615-2").as_deref(), Some("0306406152"));
        assert_eq!(digits("0-8044-2957-X").as_deref(), Some("080442957X"));
        assert_eq!(digits("0-8044-2957-x").as_deref(), Some("080442957X"));
        assert_eq!(digits("978-3-16-148410-1"), None, "check digit");
        assert_eq!(digits("X-306-40615-2"), None);
        assert_eq!(digits("978316148410"), None);
        assert_eq!(digits("tbd"), None);
        assert_eq!(digits(""), None);
    }

    #[test]
    fn validity() {
        for isbn in [
            "978-3-16-148410-0",
            "9783161484100",
            "ISBN 0-306-40615-2",
            "080442957X",
        ] {
            assert!(is_valid(isbn), "{isbn}");
        }
        for isbn in [
            "978-3-16-148410-1",
            "0-306-40615-3",
            "97831614841",
            "X783161484100",
            "978-3-16-l48410-0",
            "978316148410X",
        ] {
            assert!(!is_valid(isbn), "{isbn}");
        }
    }
}
