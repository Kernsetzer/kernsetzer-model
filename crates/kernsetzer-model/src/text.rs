//! Language-aware text normalization shared by print and EPUB output.
//!
//! Applied at render time, never stored in the model, so the author's text
//! stays as typed and changing the book language re-quotes everything.

/// Quotation style for a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quotes {
    pub open_double: char,
    pub close_double: char,
    pub open_single: char,
    pub close_single: char,
}

impl Quotes {
    /// “English” ‘style’.
    pub const EN: Self = Self {
        open_double: '\u{201C}',
        close_double: '\u{201D}',
        open_single: '\u{2018}',
        close_single: '\u{2019}',
    };
    /// „Deutsche“ ‚Anführungszeichen‘.
    pub const DE: Self = Self {
        open_double: '\u{201E}',
        close_double: '\u{201C}',
        open_single: '\u{201A}',
        close_single: '\u{2018}',
    };
    /// »Guillemets«, as preferred by many German publishers.
    pub const DE_GUILLEMETS: Self = Self {
        open_double: '\u{00BB}',
        close_double: '\u{00AB}',
        open_single: '\u{203A}',
        close_single: '\u{2039}',
    };

    pub fn for_lang(primary: &str) -> Self {
        match primary {
            "de" => Self::DE,
            _ => Self::EN,
        }
    }
}

const APOSTROPHE: char = '\u{2019}';
const NBSP: char = '\u{00A0}';
/// Narrow no-break space, between two quote marks that would touch.
const NNBSP: char = '\u{202F}';
const EN_DASH: char = '\u{2013}';
const EM_DASH: char = '\u{2014}';
const ELLIPSIS: char = '\u{2026}';
/// The author's own hyphenation point: invisible, so the rules look
/// through it (`Peter\u{ad}'s` has an apostrophe).
const SOFT_HYPHEN: char = '\u{00AD}';

/// Options for [`smarten`].
#[derive(Debug, Clone, Copy)]
pub struct SmartenOptions {
    pub quotes: Quotes,
    /// German style: ` - ` and `--` become a spaced en dash (Gedankenstrich).
    /// English style: `--`/`---` become a closed em dash.
    pub german_dashes: bool,
}

impl SmartenOptions {
    pub fn for_lang(primary: &str) -> Self {
        Self {
            quotes: Quotes::for_lang(primary),
            german_dashes: primary == "de",
        }
    }
}

/// What [`smarten`] carries from one run of a paragraph to the next (the
/// text between formatting changes), so quotes after a change open and
/// close correctly. Each paragraph, title or caption starts from
/// `Carry::default()`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Carry {
    /// The last character written, which decides whether a quote opens or
    /// closes; `None` at the start.
    pub prev: Option<char>,
    /// Whether a single quote is open: a `'` after a letter or a dash then
    /// closes it; otherwise it is an elision (German "hab'", "Andreas'") or,
    /// after a dash, opens one.
    pub single_open: bool,
    /// Whether a double quote is open: a `"` right after a dash then closes
    /// it („Ich kam –“); otherwise it opens one (He paused—“Fine.”).
    pub double_open: bool,
}

/// Convert typewriter punctuation to typographic punctuation.
///
/// `carry` holds what the runs of the paragraph before `s` left (see
/// [`Carry`]) and is updated for the next one.
pub fn smarten(s: &str, carry: &mut Carry, opts: SmartenOptions) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    let mut last: Option<char> = carry.prev;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars[i + 1..].iter().copied().find(|&n| n != SOFT_HYPHEN);
        let emitted: Option<char> = match c {
            // A quote right after the opening quote of the other kind opens
            // too ("'Hello,' she said" → “‘Hello,’ she said”).
            '"' if opens_quote(last, carry.double_open)
                || last == Some(opts.quotes.open_single) =>
            {
                carry.double_open = true;
                Some(opts.quotes.open_double)
            }
            '"' => {
                // Single quotes nest inside double ones: one left open
                // (a typo) ends here and cannot swallow a later elision.
                carry.double_open = false;
                carry.single_open = false;
                Some(opts.quotes.close_double)
            }
            '\'' => {
                // Apostrophe inside a word (don't, Peter's), before a digit
                // ('90s) or after a letter while no single quote is open
                // (hab', Andreas'); otherwise a single quote. English closes
                // with ’ either way; German needs the difference (‘ or ‹).
                let after_letter = last.is_some_and(char::is_alphabetic);
                let in_word = last.is_some_and(char::is_alphanumeric)
                    && next.is_some_and(char::is_alphanumeric);
                let elision = (opens(last) && next.is_some_and(|n| n.is_ascii_digit()))
                    || (after_letter && !carry.single_open);
                Some(if in_word || elision {
                    APOSTROPHE
                } else if opens_quote(last, carry.single_open)
                    || last == Some(opts.quotes.open_double)
                {
                    carry.single_open = true;
                    opts.quotes.open_single
                } else {
                    carry.single_open = false;
                    opts.quotes.close_single
                })
            }
            '.' if next == Some('.') && chars.get(i + 2) == Some(&'.') => {
                i += 2;
                Some(ELLIPSIS)
            }
            '-' if next == Some('-') => {
                // `--` or `---`: skip the dashes and any spaces around them.
                i += if chars.get(i + 2) == Some(&'-') { 3 } else { 2 };
                while chars.get(i) == Some(&' ') {
                    i += 1;
                }
                while out.ends_with(' ') {
                    out.pop();
                }
                if opts.german_dashes {
                    // Gedankenstrich: spaced en dash, glued to the word
                    // before it so a line never starts with the dash. That
                    // word may end the run before (Er war *müde* -- sehr);
                    // a space ending it stays. At the start of the paragraph
                    // or a line the dash opens dialogue and stands alone.
                    let word_before = carry.prev.is_some_and(|p| !p.is_whitespace());
                    if !out.is_empty() || word_before {
                        out.push(NBSP);
                    }
                    out.push(EN_DASH);
                    // A closing dash takes the punctuation or quote after
                    // it directly: "hineinging –, dann", „Ich kam –“.
                    if chars.get(i).is_some_and(|&n| closes_dash(n, carry)) {
                        last = Some(EN_DASH);
                    } else {
                        out.push(' ');
                        last = Some(' ');
                    }
                } else {
                    // English: closed em dash.
                    out.push(EM_DASH);
                    last = Some(EM_DASH);
                }
                continue;
            }
            '-' if opts.german_dashes && last == Some(' ') && next == Some(' ') => {
                glue_trailing_space(&mut out);
                Some(EN_DASH)
            }
            // A dash typed with spaces around it (Word autocorrects ` - `
            // to ` – `), or closing before punctuation or a quote, stays on
            // the line of the word before it.
            EN_DASH | EM_DASH
                if last == Some(' ')
                    && next.is_none_or(|n| n.is_whitespace() || closes_dash(n, carry)) =>
            {
                glue_trailing_space(&mut out);
                Some(c)
            }
            _ => Some(c),
        };
        if c == SOFT_HYPHEN {
            out.push(c);
        } else if let Some(e) = emitted {
            // Two quote marks never touch: ’” ‘“ „‚ »› and the like stand a
            // narrow no-break space apart.
            if is_quote_mark(e) && last.is_some_and(is_quote_mark) {
                out.push(NNBSP);
            }
            out.push(e);
            last = Some(e);
        }
        i += 1;
    }
    carry.prev = last;
    out
}

/// Everything print and EPUB apply to a run of manuscript text: [`smarten`]
/// for the language, then for German [`german_nbsp`]. `primary` is the
/// primary language subtag ([`crate::Lang::primary`]); `carry` as for
/// [`smarten`].
pub fn normalize(s: &str, carry: &mut Carry, primary: &str) -> String {
    normalize_with(s, carry, primary, SmartenOptions::for_lang(primary))
}

/// [`normalize`] with other options than the language's own, e.g.
/// [`Quotes::DE_GUILLEMETS`] for a German book set with guillemets.
pub fn normalize_with(s: &str, carry: &mut Carry, primary: &str, opts: SmartenOptions) -> String {
    let s = smarten(s, carry, opts);
    if primary == "de" { german_nbsp(&s) } else { s }
}

/// Turn the space just written before a spaced dash into a no-break space.
fn glue_trailing_space(out: &mut String) {
    if out.ends_with(' ') {
        out.pop();
        out.push(NBSP);
    }
}

/// A quotation mark of any style, or the apostrophe.
fn is_quote_mark(c: char) -> bool {
    matches!(
        c,
        '\u{2018}'
            | '\u{2019}'
            | '\u{201A}'
            | '\u{201C}'
            | '\u{201D}'
            | '\u{201E}'
            | '\u{00AB}'
            | '\u{00BB}'
            | '\u{2039}'
            | '\u{203A}'
    )
}

/// Whether a quote after `prev` is an opening quote.
fn opens(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(p) => {
            p.is_whitespace()
                || matches!(p, '(' | '[' | '{' | '\u{2014}' | '\u{2013}' | '/')
                || p == NBSP
        }
    }
}

/// Whether a quote after `prev` opens, `open` saying whether a quote of its
/// kind is open. Right after a dash it can do either, so it closes an open
/// one („Ich kam –“) and otherwise opens (He paused—“Fine.”).
fn opens_quote(prev: Option<char>, open: bool) -> bool {
    match prev {
        Some(EN_DASH | EM_DASH) => !open,
        p => opens(p),
    }
}

/// Whether `c` right after a dash ends with it: punctuation, or a quote that
/// closes ("hineinging –, dann", „Ich kam –“).
fn closes_dash(c: char, carry: &Carry) -> bool {
    ",.;:)]?!".contains(c) || (c == '"' && carry.double_open) || (c == '\'' && carry.single_open)
}

/// German: non-breaking (thin would be nicer, NBSP is safer across readers)
/// spaces inside common abbreviations and between numbers and units.
pub fn german_nbsp(s: &str) -> String {
    const ABBR: &[&str] = &[
        "z. B.", "d. h.", "u. a.", "o. Ä.", "u. Ä.", "v. a.", "s. o.", "s. u.", "z. T.", "i. d. R.",
    ];
    let mut out = s.to_string();
    for a in ABBR {
        if out.contains(a) {
            out = out.replace(a, &a.replace(' ', "\u{00A0}"));
        }
    }
    // "10 km", "S. 12", "Nr. 5": number/unit glue.
    let chars: Vec<char> = out.chars().collect();
    let mut res = String::with_capacity(out.len());
    for (idx, &c) in chars.iter().enumerate() {
        if c == ' ' && idx > 0 {
            let before = chars[idx - 1];
            let after = chars.get(idx + 1).copied();
            let num_then_word = before.is_ascii_digit() && after.is_some_and(char::is_alphabetic);
            let prefix_then_num = (ends_with(&chars[..idx], "S.")
                || ends_with(&chars[..idx], "Nr.")
                || ends_with(&chars[..idx], "§"))
                && after.is_some_and(|a| a.is_ascii_digit());
            if num_then_word || prefix_then_num {
                res.push(NBSP);
                continue;
            }
        }
        res.push(c);
    }
    res
}

fn ends_with(chars: &[char], suffix: &str) -> bool {
    let suf: Vec<char> = suffix.chars().collect();
    chars.len() >= suf.len() && chars[chars.len() - suf.len()..] == suf[..]
}

/// Where a chapter's first paragraph splits for a drop cap, as byte offsets
/// into `s`: `(initial_end, lead_in_end)`. The initial runs up to and
/// including the first letter or digit (so it carries any opening quote
/// before it); the lead-in is the rest of that word and two more
/// whitespace-separated words (a lone dash counts as a word), or all of `s`
/// if there are fewer. `None` when `s` has no letter or digit. Print and
/// EPUB both split with this, so they cannot disagree; each decides for
/// itself what may stand before the letter.
pub fn opening_split(s: &str) -> Option<(usize, usize)> {
    let (i, c) = s.char_indices().find(|(_, c)| c.is_alphanumeric())?;
    let initial_end = i + c.len_utf8();
    let after = &s[initial_end..];
    let (mut words, mut in_word) = (0, false);
    for (i, c) in after.char_indices() {
        if c.is_whitespace() {
            if in_word {
                words += 1;
                if words == 3 {
                    return Some((initial_end, initial_end + i));
                }
            }
            in_word = false;
        } else {
            in_word = true;
        }
    }
    Some((initial_end, s.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(s: &str) -> String {
        smarten(s, &mut Carry::default(), SmartenOptions::for_lang("en"))
    }
    fn de(s: &str) -> String {
        smarten(s, &mut Carry::default(), SmartenOptions::for_lang("de"))
    }
    /// A run that follows `c` in its paragraph, no quote open.
    fn after(c: char) -> Carry {
        Carry {
            prev: Some(c),
            ..Carry::default()
        }
    }

    #[test]
    fn english_quotes_and_apostrophes() {
        assert_eq!(
            en(r#""Hello," she said."#),
            "\u{201C}Hello,\u{201D} she said."
        );
        assert_eq!(en("don't"), "don\u{2019}t");
        assert_eq!(en("'Tis the '90s"), "\u{2018}Tis the \u{2019}90s");
        assert_eq!(en("He said 'yes'."), "He said \u{2018}yes\u{2019}.");
    }

    #[test]
    fn german_quotes() {
        assert_eq!(de(r#"Sie sagte: "Ja.""#), "Sie sagte: \u{201E}Ja.\u{201C}");
        assert_eq!(de("Er rief 'hallo'"), "Er rief \u{201A}hallo\u{2018}");
        assert_eq!(de("geht's"), "geht\u{2019}s");
    }

    #[test]
    fn word_final_apostrophe_is_not_a_closing_quote() {
        // in German the closing single quote is ‘ (or ‹), so an
        // elision at the end of a word must not take it.
        assert_eq!(de("ich war' im Stalle"), "ich war\u{2019} im Stalle");
        assert_eq!(de("Ich stand' die 47"), "Ich stand\u{2019} die 47");
        assert_eq!(de("Andreas' Hut."), "Andreas\u{2019} Hut.");
        assert_eq!(de("Das war's."), "Das war\u{2019}s.");
        // An open single quote still closes after a letter, also after an
        // elision in the same paragraph.
        assert_eq!(
            de(
                "Ich war' im Stalle. Wärme und Geruch kam hervor: 'eine dringende Reise', stand' ich."
            ),
            "Ich war\u{2019} im Stalle. Wärme und Geruch kam hervor: \
             \u{201A}eine dringende Reise\u{2018}, stand\u{2019} ich."
        );
        assert_eq!(
            de("\"Ich war' 'so ist es' gewesen\", sagte er."),
            "\u{201E}Ich war\u{2019} \u{201A}so ist es\u{2018} gewesen\u{201C}, sagte er."
        );
        // A closing double quote closes a single quote left open inside it.
        assert_eq!(
            de("\"Er rief 'Gut genug!\", und ich war' gewartet."),
            "\u{201E}Er rief \u{201A}Gut genug!\u{201C}, und ich war\u{2019} gewartet."
        );
        let guillemets = SmartenOptions {
            quotes: Quotes::DE_GUILLEMETS,
            ..SmartenOptions::for_lang("de")
        };
        assert_eq!(
            smarten(
                "ich war' im Stalle, 'eine dringende Reise'",
                &mut Carry::default(),
                guillemets
            ),
            "ich war\u{2019} im Stalle, \u{203A}eine dringende Reise\u{2039}"
        );
        // English closes single quotes with ’ anyway: nothing changes.
        assert_eq!(
            en("I'm goin' home, the boys' hats, 'yes', 'Tis"),
            "I\u{2019}m goin\u{2019} home, the boys\u{2019} hats, \
             \u{2018}yes\u{2019}, \u{2018}Tis"
        );
    }

    #[test]
    fn soft_hyphens_are_invisible_to_the_rules() {
        assert_eq!(
            en("Peter\u{ad}'s \"Ver\u{ad}legenheit\""),
            "Peter\u{ad}’s “Ver\u{ad}legenheit”"
        );
        assert_eq!(de("\"Ver\u{ad}legenheit\""), "„Ver\u{ad}legenheit“");
        assert_eq!(en("don\u{ad}'t"), "don\u{ad}’t");
    }

    #[test]
    fn dashes() {
        assert_eq!(en("wait -- what"), "wait\u{2014}what");
        assert_eq!(en("wait---what"), "wait\u{2014}what");
        assert_eq!(de("Er kam -- spät"), "Er kam\u{A0}\u{2013} spät");
        assert_eq!(de("Er kam - spät"), "Er kam\u{A0}\u{2013} spät");
        assert_eq!(de("Ost-West"), "Ost-West");
    }

    #[test]
    fn a_closing_dash_takes_punctuation_directly() {
        // no space between the Gedankenstrich and a comma after it.
        assert_eq!(
            de("ohne dass je jemand hineinging --, dann ein Kreisverkehr"),
            "ohne dass je jemand hineinging\u{A0}\u{2013}, dann ein Kreisverkehr"
        );
        for p in [",", ".", ";", ":", ")", "]", "?", "!"] {
            assert_eq!(de(&format!("ja --{p}")), format!("ja\u{A0}\u{2013}{p}"));
            assert_eq!(de(&format!("ja -- {p}")), format!("ja\u{A0}\u{2013}{p}"));
        }
        assert_eq!(de("(liegt -- lag)"), "(liegt\u{A0}\u{2013} lag)");
        // English dashes are closed anyway.
        assert_eq!(en("went in--, then"), "went in\u{2014}, then");
        assert_eq!(en("went in --, then"), "went in\u{2014}, then");
        assert_eq!(en("(not --)"), "(not\u{2014})");
    }

    #[test]
    fn a_dash_after_a_formatting_run_keeps_its_space() {
        // Er war *müde* -- sehr müde: the word before the dash is in the run
        // before, and the space between them must not get lost.
        let joined = |runs: &[&str], lang: &str| {
            let opts = SmartenOptions::for_lang(lang);
            let mut carry = Carry::default();
            runs.iter()
                .map(|r| smarten(r, &mut carry, opts))
                .collect::<String>()
        };
        assert_eq!(
            joined(&["Er war ", "müde", " -- sehr müde."], "de"),
            "Er war müde\u{A0}\u{2013} sehr müde."
        );
        assert_eq!(
            joined(&["Er war ", "müde", "-- sehr müde."], "de"),
            "Er war müde\u{A0}\u{2013} sehr müde."
        );
        assert_eq!(
            joined(&["\"Ich war ", "müde", " --\""], "de"),
            "\u{201E}Ich war müde\u{A0}\u{2013}\u{201C}"
        );
        // A space that ends the run before stays as it is, not doubled.
        assert_eq!(
            joined(&["Er war ", "müde ", "-- sehr müde."], "de"),
            "Er war müde \u{2013} sehr müde."
        );
        // A dialogue dash at the start or after a line break is not glued.
        assert_eq!(joined(&["-- Ja."], "de"), "\u{2013} Ja.");
        assert_eq!(
            smarten("-- Ja.", &mut after(' '), SmartenOptions::for_lang("de")),
            "\u{2013} Ja."
        );
        // English dashes are closed.
        assert_eq!(
            joined(&["He was ", "tired", " -- very."], "en"),
            "He was tired\u{2014}very."
        );
    }

    #[test]
    fn a_quote_after_a_dash_closes_an_open_one() {
        // Speech that breaks off: the quote after the dash closes it, with
        // no space between them.
        assert_eq!(
            de("\"Ich kam --\" sagte er."),
            "\u{201E}Ich kam\u{A0}\u{2013}\u{201C} sagte er."
        );
        assert_eq!(
            de("\"Aber ich --\""),
            "\u{201E}Aber ich\u{A0}\u{2013}\u{201C}"
        );
        assert_eq!(
            de("Sie sagt 'nie --', dann geht sie."),
            "Sie sagt \u{201A}nie\u{A0}\u{2013}\u{2018}, dann geht sie."
        );
        assert_eq!(
            en("\"I was --\" he said."),
            "\u{201C}I was\u{2014}\u{201D} he said."
        );
        assert_eq!(
            en("'I was--', he said."),
            "\u{2018}I was\u{2014}\u{2019}, he said."
        );
        // Typed dashes too, glued to the word before them like the others.
        assert_eq!(
            de("\"Ich kam \u{2013}\", sagte er."),
            "\u{201E}Ich kam\u{A0}\u{2013}\u{201C}, sagte er."
        );
        assert_eq!(
            de("hineinging \u{2013}, dann"),
            "hineinging\u{A0}\u{2013}, dann"
        );
        assert_eq!(
            en("\"I was\u{2014}\" he said."),
            "\u{201C}I was\u{2014}\u{201D} he said."
        );
        // With no quote of its kind open, a quote after a dash opens one.
        assert_eq!(
            de("\"Ja\", sagt er -- \"nein\"."),
            "\u{201E}Ja\u{201C}, sagt er\u{A0}\u{2013} \u{201E}nein\u{201C}."
        );
        assert_eq!(
            en("He paused--\"Fine.\""),
            "He paused\u{2014}\u{201C}Fine.\u{201D}"
        );
        assert_eq!(
            en("\"Yes\"--\"no\""),
            "\u{201C}Yes\u{201D}\u{2014}\u{201C}no\u{201D}"
        );
        assert_eq!(en("the '80s--'90s"), "the \u{2019}80s\u{2014}\u{2019}90s");
        // The open quote carries across formatting runs.
        let opts = SmartenOptions::for_lang("de");
        let mut carry = Carry::default();
        let runs = ["\"Ich kam ", "nicht", " mehr --\" sagte er."];
        let out: String = runs.iter().map(|r| smarten(r, &mut carry, opts)).collect();
        assert_eq!(
            out,
            "\u{201E}Ich kam nicht mehr\u{A0}\u{2013}\u{201C} sagte er."
        );
    }

    #[test]
    fn touching_quotes_stand_a_narrow_space_apart() {
        // A quote inside a quote at its start or end: the two marks get a
        // narrow no-break space, and the inner one opens after the outer.
        assert_eq!(
            en("\"'How doth the little--'\" and she"),
            "\u{201C}\u{202F}\u{2018}How doth the little\u{2014}\u{2019}\u{202F}\u{201D} and she"
        );
        assert_eq!(
            en("'\"Yes,\" he said.'"),
            "\u{2018}\u{202F}\u{201C}Yes,\u{201D} he said.\u{2019}"
        );
        assert_eq!(
            de("\"'Nein', sagte er, 'nie'\""),
            "\u{201E}\u{202F}\u{201A}Nein\u{2018}, sagte er, \u{201A}nie\u{2018}\u{202F}\u{201C}"
        );
        let guillemets = SmartenOptions {
            quotes: Quotes::DE_GUILLEMETS,
            german_dashes: true,
        };
        assert_eq!(
            smarten("\"'Nie'\"", &mut Carry::default(), guillemets),
            "\u{00BB}\u{202F}\u{203A}Nie\u{2039}\u{202F}\u{00AB}"
        );
        // Typed curly quotes, and an apostrophe before a closing quote.
        assert_eq!(
            en("\u{201C}\u{2018}Hi\u{2019}\u{201D}"),
            "\u{201C}\u{202F}\u{2018}Hi\u{2019}\u{202F}\u{201D}"
        );
        assert_eq!(
            en("\"the Jones'\""),
            "\u{201C}the Jones\u{2019}\u{202F}\u{201D}"
        );
        // Across formatting runs: the space starts the second run.
        let opts = SmartenOptions::for_lang("en");
        let mut carry = Carry::default();
        let runs = ["\"", "'Tis", " so.'\""];
        let out: Vec<String> = runs.iter().map(|r| smarten(r, &mut carry, opts)).collect();
        assert_eq!(
            out,
            [
                "\u{201C}",
                "\u{202F}\u{2018}Tis",
                " so.\u{2019}\u{202F}\u{201D}"
            ]
        );
        // One quote alone is untouched.
        assert_eq!(en("\"Hi.\""), "\u{201C}Hi.\u{201D}");
    }

    #[test]
    fn spaced_dash_never_starts_a_line() {
        // Typed (or autocorrected) dashes get the same no-break space.
        assert_eq!(de("Er kam \u{2013} spät"), "Er kam\u{A0}\u{2013} spät");
        assert_eq!(en("wait \u{2014} what"), "wait\u{A0}\u{2014} what");
        assert_eq!(de("Er kam \u{2013}"), "Er kam\u{A0}\u{2013}");
        // Unspaced dashes and dialogue dashes at the start are untouched.
        assert_eq!(de("\u{2013} Ja."), "\u{2013} Ja.");
        assert_eq!(de("1990\u{2013}2000"), "1990\u{2013}2000");
        assert_eq!(en("wait\u{2014}what"), "wait\u{2014}what");
    }

    #[test]
    fn ellipsis() {
        assert_eq!(en("Well..."), "Well\u{2026}");
    }

    #[test]
    fn continuation_uses_prev_char() {
        // Fragment after an italic run ending in a letter: quote closes.
        assert_eq!(
            smarten(
                "\" he said",
                &mut after('o'),
                SmartenOptions::for_lang("en")
            ),
            "\u{201D} he said"
        );
    }

    #[test]
    fn an_open_single_quote_carries_across_runs() {
        // 'eine dringende *Reise*', und ich war' …: the quote closes after
        // the italic run, the elision after it stays an apostrophe.
        let opts = SmartenOptions::for_lang("de");
        let mut carry = Carry::default();
        let runs = [
            "Er sagt: 'eine dringende ",
            "Reise",
            "', und ich war' Zeit.",
        ];
        let out: String = runs.iter().map(|r| smarten(r, &mut carry, opts)).collect();
        assert_eq!(
            out,
            "Er sagt: \u{201A}eine dringende Reise\u{2018}, und ich war\u{2019} Zeit."
        );
        assert_eq!(carry, after('.'));
    }

    #[test]
    fn normalize_is_smarten_then_german_spacing() {
        assert_eq!(
            normalize("\"z. B. 10 km\" -- ja", &mut Carry::default(), "de"),
            "\u{201E}z.\u{A0}B. 10\u{A0}km\u{201C}\u{A0}\u{2013} ja"
        );
        assert_eq!(
            normalize("z. B. 10 km", &mut Carry::default(), "en"),
            "z. B. 10 km"
        );
        assert_eq!(
            normalize("\" he said", &mut after('o'), "en"),
            "\u{201D} he said"
        );
    }

    #[test]
    fn german_nbsp_rules() {
        assert_eq!(german_nbsp("z. B. 10 km"), "z.\u{A0}B. 10\u{A0}km");
        assert_eq!(german_nbsp("siehe S. 12"), "siehe S.\u{A0}12");
    }

    #[test]
    fn opening_splits() {
        let split = |s: &'static str| opening_split(s).map(|(a, b)| (&s[..a], &s[a..b]));
        assert_eq!(
            split("It is a truth universally acknowledged"),
            Some(("I", "t is a"))
        );
        // Opening quotes go with the initial; German and guillemets alike.
        assert_eq!(
            split("\"Wer kann das\", sagte sie"),
            Some(("\"W", "er kann das\","))
        );
        assert_eq!(
            split("\u{201E}Wer kann das\u{201C}, sagte sie"),
            Some(("\u{201E}W", "er kann das\u{201C},"))
        );
        assert_eq!(
            split("\u{BB}Ja\u{AB}, sagte er"),
            Some(("\u{BB}J", "a\u{AB}, sagte er"))
        );
        // An apostrophe stays in the lead-in, a leading one in the initial.
        assert_eq!(
            split("Don't keep coughing so, Kitty"),
            Some(("D", "on't keep coughing"))
        );
        assert_eq!(split("'Tis the season"), Some(("'T", "is the season")));
        // A digit is an initial.
        assert_eq!(split("1984 was a year"), Some(("1", "984 was a")));
        // One-letter words: the initial is the whole first word.
        assert_eq!(
            split("A single man of large fortune"),
            Some(("A", " single man of"))
        );
        assert_eq!(split("I"), Some(("I", "")));
        // A lone dash counts as a word.
        assert_eq!(split("So \u{2013} he said"), Some(("S", "o \u{2013} he")));
        // Leading whitespace and fewer than three words.
        assert_eq!(split("  Indented start"), Some(("  I", "ndented start")));
        assert_eq!(split("Short."), Some(("S", "hort.")));
        // Nothing to split.
        for none in ["", "   ", "\u{2014}", "* * *", "\u{201E}\u{201C}"] {
            assert_eq!(opening_split(none), None, "{none:?}");
        }
    }

    #[test]
    fn normalize_takes_smarten_options() {
        let opts = SmartenOptions {
            quotes: Quotes::DE_GUILLEMETS,
            ..SmartenOptions::for_lang("de")
        };
        let mut carry = Carry::default();
        assert_eq!(
            normalize_with("\"z. B. 10 km\" -- ja", &mut carry, "de", opts),
            "\u{BB}z.\u{A0}B. 10\u{A0}km\u{AB}\u{A0}\u{2013} ja"
        );
    }
}
