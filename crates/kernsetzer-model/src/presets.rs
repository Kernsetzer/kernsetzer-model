//! Trim sizes and retailer margin rules, as data.
//!
//! Retailers change these occasionally. Each rule block below names the pages
//! it was taken from; [`SPECS_VERIFIED_ON`] is the day they were last compared
//! with the live pages. Re-verify before every release.

use serde::{Deserialize, Serialize};

const MM_PER_INCH: f32 = 25.4;

/// Day the retailer numbers in this file were last checked against the
/// sources named next to them (ISO 8601).
pub const SPECS_VERIFIED_ON: &str = "2026-10-21";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrimSize {
    pub width_mm: f32,
    pub height_mm: f32,
}

impl TrimSize {
    pub const fn mm(width_mm: f32, height_mm: f32) -> Self {
        Self {
            width_mm,
            height_mm,
        }
    }

    pub const fn inches(w: f32, h: f32) -> Self {
        Self::mm(w * MM_PER_INCH, h * MM_PER_INCH)
    }

    pub const US_5X8: Self = Self::inches(5.0, 8.0);
    pub const US_5_25X8: Self = Self::inches(5.25, 8.0);
    pub const US_5_5X8_5: Self = Self::inches(5.5, 8.5);
    pub const US_6X9: Self = Self::inches(6.0, 9.0);
    pub const A5: Self = Self::mm(148.0, 210.0);
    pub const DE_12X19: Self = Self::mm(120.0, 190.0);
    pub const DE_13_5X21_5: Self = Self::mm(135.0, 215.0);
    pub const DE_15_5X22: Self = Self::mm(155.0, 220.0);
    pub const DE_17X22: Self = Self::mm(170.0, 220.0);
    pub const DE_17X17: Self = Self::mm(170.0, 170.0);
    /// Landscape, as BoD lists it (21 wide, 15 high).
    pub const DE_21X15: Self = Self::mm(210.0, 150.0);
    pub const DE_19X27: Self = Self::mm(190.0, 270.0);
    pub const DE_21X21: Self = Self::mm(210.0, 210.0);
    pub const DE_21X29_7: Self = Self::mm(210.0, 297.0);

    /// Named presets shown in the UI.
    pub fn presets() -> &'static [(&'static str, TrimSize)] {
        &[
            ("5 × 8 in", Self::US_5X8),
            ("5.25 × 8 in", Self::US_5_25X8),
            ("5.5 × 8.5 in", Self::US_5_5X8_5),
            ("6 × 9 in", Self::US_6X9),
            ("A5 (148 × 210 mm)", Self::A5),
            ("12 × 19 cm", Self::DE_12X19),
            ("13.5 × 21.5 cm", Self::DE_13_5X21_5),
        ]
    }
}

/// Minimum margins a retailer requires, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MinMargins {
    pub inside_mm: f32,
    pub outside_mm: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Retailer {
    Kdp,
    IngramSpark,
    /// Books on Demand (Germany).
    Bod,
    Generic,
}

/// Bleed a retailer wants on interior pages.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bleed {
    pub mm: f32,
    /// Also on the binding (inside) edge. KDP and IngramSpark want bleed on
    /// the top, bottom and outside edge only.
    pub on_gutter: bool,
}

/// Page counts a retailer prints (paperback, black ink on white paper).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageLimits {
    pub min: u32,
    pub max: u32,
}

impl PageLimits {
    pub fn contains(self, pages: u32) -> bool {
        (self.min..=self.max).contains(&pages)
    }
}

// KDP, checked 2026-10-21 against:
//   https://kdp.amazon.com/en_US/help/topic/GVBQ3CMEQW3W2VL6
//     ("Set Trim Size, Bleed, and Margins")
//   https://kdp.amazon.com/help/topic/G201857950
//     ("Paperback Submission Guidelines")
//   https://kdp.amazon.com/de_DE/help/topic/GVBQ3CMEQW3W2VL6
//     (German store; same table, in mm)
// Nothing changed since the table was first encoded. The pages carry no
// revision date. The text above the margin table says 0.25 in = 6.35 mm, the
// table rounds to 6.4 mm; the inch values are what KDP's checker uses.

/// KDP minimum inside (gutter) margin by page count: (up to this many
/// pages, margin in inches). Longer books than the last row (KDP's
/// paperback maximum) use the last row.
pub const KDP_INSIDE_MARGINS_IN: &[(u32, f32)] = &[
    (150, 0.375),
    (300, 0.5),
    (500, 0.625),
    (700, 0.75),
    (828, 0.875),
];

/// KDP minimum outside margin (top, bottom, outer edge) in inches, without
/// and with bleed.
pub const KDP_OUTSIDE_MARGIN_IN: f32 = 0.25;
pub const KDP_OUTSIDE_MARGIN_BLEED_IN: f32 = 0.375;

/// KDP bleed in inches, beyond the trim on top, bottom and outside edge
/// only (a 6 × 9 in trim becomes 6.125 × 9.25 in). If one page bleeds, the
/// whole interior file must carry bleed.
pub const KDP_BLEED_IN: f32 = 0.125;

/// KDP paperback page-count limits. The minimum is 24 for black ink; standard
/// colour ink starts at 72. The maximum is for black ink on white paper on
/// most trim sizes; cream paper stops at 776, groundwood at 812, and a few
/// large trims are lower (8.5 × 8.5 and 8.5 × 11 in: 590 on white).
pub const KDP_MIN_PAGES: u32 = 24;
pub const KDP_MAX_PAGES: u32 = 828;

// IngramSpark, checked 2026-10-21 against:
//   https://www.ingramspark.com/hubfs/downloads/file-creation-guide.pdf
//     (File Creation Guide, dated 8.24.26: black & white text, quick reference)
//   https://www.ingramspark.com/hubfs/downloads/trim-sizes.pdf
//     (trim size matrix: page ranges, custom trim sizes)
// Margin and bleed unchanged. The guide *recommends* at least 0.5 in (13 mm)
// on all sides of the text block, page numbers included; it has no separate
// or page-count-dependent gutter figure (blog posts that say otherwise are
// quoting KDP's table), and it allows 1/16 in of printing variance. Treated
// as the minimum here. Not checked: the help-centre article "Resolving
// Common File Issues" (HTTP 403 from here), which might state a hard limit
// below the recommendation.

/// IngramSpark recommended minimum margin on every side, in inches.
pub const INGRAM_MARGIN_IN: f32 = 0.5;

/// IngramSpark bleed in inches: 0.125 in (3 mm) on the three trim edges (top,
/// bottom, outside), none on the bind edge. A book without bleed is
/// submitted at trim size.
pub const INGRAM_BLEED_IN: f32 = 0.125;

/// IngramSpark paperback (perfect bound) page-count limits from the trim size
/// matrix: 18 minimum; the maximum is for black ink on 50 lb white paper.
/// Crème 50 lb stops at 1050; groundwood 38 lb and white 70 lb at 900. Text
/// files are stored with an even page count (a blank page is added).
pub const INGRAM_MIN_PAGES: u32 = 18;
pub const INGRAM_MAX_PAGES: u32 = 1200;

// BoD (Books on Demand, Germany), checked 2026-10-21 against:
//   https://www.bod.de/bodfiles/DE-Storage/documents/bod-autorenbroschuere.pdf
//     (author brochure: the ten standard formats)
//   https://www.bod.de/buch/buch-gestalten
//     (bleed: "5 mm, 12 x 19 cm becomes 13 x 20 cm", no crop marks)
//   https://blog.bod.de/gestalten/zehn-grundregeln-fuer-ein-professionelles-buchblockdesign/
//     (margin order; bleed "an jeder Seite" 5 mm in a staff reply, 2024-09-04)
//   https://blog.bod.de/schreiben/das-einrichten-des-buchblocks-mit-microsoft-word/
//     (example margins; staff reply of 2025-07-17: keep non-bleeding content
//      at least 5 mm inside the page edge)
//   https://bod.de/buch/buch-drucken and
//   https://blog.bod.de/gestalten/buchformat-softcover-alles-was-du-wissen-musst/
//     (paperback and hardcover: 24 to 900 pages, divisible by two)
// BoD publishes NO minimum-margin table and no gutter rule that scales with
// page count. What exists: the 5 mm guidance above (a staff comment, and it
// does not say whether the binding edge is included), and a tutorial example
// of 1.7 cm top, bottom and inside, 2 cm outside, which BoD calls an example,
// not a requirement. The minimum below is that 5 mm; a usable gutter for
// thick books is a typographic choice made on top of it. Page limits
// conflict between pages: an older post (2024-04-16) says 90 g paper holds
// "up to 700 pages", the publisher page lists 1,050 (90 g white) and 900
// (90 g crème); the author-facing 24 to 900 is used. The orientation of A4
// is unclear in the brochure (listed 21 x 29.7 once, 29.7 x 21 once);
// portrait is used. Custom sizes: the publisher page gives 10.8 x 17 cm to
// 21.6 x 29.7 cm in 1 mm steps, the author pages offer only the ten below.

/// BoD's ten standard formats (width × height), as listed in the author
/// brochure: seven standard, two large (19 × 27, 21 × 21) and one maxi
/// (21 × 29.7).
pub const BOD_TRIM_SIZES: &[(&str, TrimSize)] = &[
    ("12 × 19 cm", TrimSize::DE_12X19),
    ("13.5 × 21.5 cm", TrimSize::DE_13_5X21_5),
    ("A5 (148 × 210 mm)", TrimSize::A5),
    ("15.5 × 22 cm", TrimSize::DE_15_5X22),
    ("17 × 22 cm", TrimSize::DE_17X22),
    ("17 × 17 cm", TrimSize::DE_17X17),
    ("21 × 15 cm", TrimSize::DE_21X15),
    ("19 × 27 cm", TrimSize::DE_19X27),
    ("21 × 21 cm", TrimSize::DE_21X21),
    ("21 × 29.7 cm", TrimSize::DE_21X29_7),
];

/// BoD bleed in millimetres: 5 mm on every side, so 12 × 19 cm is delivered
/// as 13 × 20 cm. Only needed when content runs to the page edge.
pub const BOD_BLEED_MM: f32 = 5.0;

/// BoD minimum distance of content from the page edge, in millimetres (see
/// the note above: guidance from a staff comment, not a specification).
pub const BOD_MIN_MARGIN_MM: f32 = 5.0;

/// BoD paperback and hardcover page-count limits; the count must be even
/// (blank pages are added and approved in the print preview otherwise).
pub const BOD_MIN_PAGES: u32 = 24;
pub const BOD_MAX_PAGES: u32 = 900;

impl Retailer {
    /// Minimum margins for a book with `pages` pages.
    pub fn min_margins(self, pages: u32, bleed: bool) -> MinMargins {
        match self {
            Retailer::Kdp => {
                let inside_in = KDP_INSIDE_MARGINS_IN
                    .iter()
                    .find(|(max, _)| pages <= *max)
                    .or(KDP_INSIDE_MARGINS_IN.last())
                    .map_or(0.875, |(_, m)| *m);
                let outside_in = if bleed {
                    KDP_OUTSIDE_MARGIN_BLEED_IN
                } else {
                    KDP_OUTSIDE_MARGIN_IN
                };
                MinMargins {
                    inside_mm: inside_in * MM_PER_INCH,
                    outside_mm: outside_in * MM_PER_INCH,
                }
            }
            // IngramSpark recommends 0.5 in all round; treat as minimum.
            Retailer::IngramSpark => MinMargins {
                inside_mm: INGRAM_MARGIN_IN * MM_PER_INCH,
                outside_mm: INGRAM_MARGIN_IN * MM_PER_INCH,
            },
            Retailer::Bod => MinMargins {
                inside_mm: BOD_MIN_MARGIN_MM,
                outside_mm: BOD_MIN_MARGIN_MM,
            },
            Retailer::Generic => MinMargins {
                inside_mm: 15.0,
                outside_mm: 10.0,
            },
        }
    }

    /// Bleed for interiors with content that runs to the page edge. Generic
    /// is Kernsetzer's own default: 3 mm on the three outer edges.
    pub fn bleed(self) -> Bleed {
        match self {
            Retailer::Kdp => Bleed {
                mm: KDP_BLEED_IN * MM_PER_INCH,
                on_gutter: false,
            },
            Retailer::IngramSpark => Bleed {
                mm: INGRAM_BLEED_IN * MM_PER_INCH,
                on_gutter: false,
            },
            Retailer::Bod => Bleed {
                mm: BOD_BLEED_MM,
                on_gutter: true,
            },
            Retailer::Generic => Bleed {
                mm: 3.0,
                on_gutter: false,
            },
        }
    }

    /// Page counts the retailer prints; `None` where Kernsetzer states no limit.
    pub fn page_limits(self) -> Option<PageLimits> {
        let (min, max) = match self {
            Retailer::Kdp => (KDP_MIN_PAGES, KDP_MAX_PAGES),
            Retailer::IngramSpark => (INGRAM_MIN_PAGES, INGRAM_MAX_PAGES),
            Retailer::Bod => (BOD_MIN_PAGES, BOD_MAX_PAGES),
            Retailer::Generic => return None,
        };
        Some(PageLimits { min, max })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Retailer; 4] = [
        Retailer::Kdp,
        Retailer::IngramSpark,
        Retailer::Bod,
        Retailer::Generic,
    ];

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    fn inside_in(pages: u32) -> f32 {
        Retailer::Kdp.min_margins(pages, false).inside_mm / MM_PER_INCH
    }

    #[test]
    fn six_by_nine_in_mm() {
        assert!((TrimSize::US_6X9.width_mm - 152.4).abs() < 0.01);
        assert!((TrimSize::US_6X9.height_mm - 228.6).abs() < 0.01);
    }

    #[test]
    fn kdp_inside_margin_grows_with_pages() {
        let small = Retailer::Kdp.min_margins(120, false);
        let big = Retailer::Kdp.min_margins(420, false);
        assert!((small.inside_mm - 9.525).abs() < 0.01);
        assert!((big.inside_mm - 15.875).abs() < 0.01);
        assert!(Retailer::Kdp.min_margins(10_000, true).outside_mm > small.outside_mm);
    }

    #[test]
    fn kdp_table_is_well_formed() {
        assert!(!KDP_INSIDE_MARGINS_IN.is_empty());
        for pair in KDP_INSIDE_MARGINS_IN.windows(2) {
            let ((p0, m0), (p1, m1)) = (pair[0], pair[1]);
            assert!(p0 < p1, "page limits must ascend: {p0} {p1}");
            assert!(m0 < m1, "margins must grow: {m0} {m1}");
        }
        assert!(KDP_INSIDE_MARGINS_IN.iter().all(|(_, m)| *m > 0.0));
    }

    #[test]
    fn kdp_rows_apply_up_to_and_including_their_limit() {
        // KDP's published rows: 24–150, 151–300, 301–500, 501–700, 701–828.
        let expected = [
            (1, 0.375),
            (24, 0.375),
            (150, 0.375),
            (151, 0.5),
            (300, 0.5),
            (301, 0.625),
            (500, 0.625),
            (501, 0.75),
            (700, 0.75),
            (701, 0.875),
            (828, 0.875),
        ];
        for (pages, margin) in expected {
            assert!(
                close(inside_in(pages), margin),
                "{pages} pages: {}",
                inside_in(pages)
            );
        }
        for (i, &(max, margin)) in KDP_INSIDE_MARGINS_IN.iter().enumerate() {
            assert!(close(inside_in(max), margin));
            if let Some(&(_, next)) = KDP_INSIDE_MARGINS_IN.get(i + 1) {
                assert!(close(inside_in(max + 1), next), "{} pages", max + 1);
            }
        }
    }

    #[test]
    fn kdp_page_limits_frame_the_margin_table() {
        assert_eq!(KDP_MIN_PAGES, 24);
        assert_eq!(KDP_MAX_PAGES, 828);
        assert_eq!(
            KDP_INSIDE_MARGINS_IN.last().map(|r| r.0),
            Some(KDP_MAX_PAGES)
        );
        assert!(KDP_INSIDE_MARGINS_IN[0].0 > KDP_MIN_PAGES);
    }

    #[test]
    fn kdp_bleed_is_an_eighth_of_an_inch() {
        assert!(close(KDP_BLEED_IN, 0.125));
        assert!(close(KDP_BLEED_IN * MM_PER_INCH, 3.175));
        // KDP's own example: 6 × 9 in plus bleed is 6.125 × 9.25 in.
        let bleed = |trim: TrimSize| {
            (
                (trim.width_mm + KDP_BLEED_IN * MM_PER_INCH) / MM_PER_INCH,
                (trim.height_mm + 2.0 * KDP_BLEED_IN * MM_PER_INCH) / MM_PER_INCH,
            )
        };
        let (w, h) = bleed(TrimSize::US_6X9);
        assert!(close(w, 6.125) && close(h, 9.25), "{w} × {h}");
    }

    #[test]
    fn kdp_beyond_the_table_keeps_the_last_row() {
        let last = KDP_INSIDE_MARGINS_IN.last().unwrap().1;
        for pages in [829, 1_000, u32::MAX] {
            assert!(close(inside_in(pages), last));
        }
        assert!(close(inside_in(0), KDP_INSIDE_MARGINS_IN[0].1));
    }

    #[test]
    fn kdp_outside_margin_depends_on_bleed_only() {
        for pages in [0, 24, 150, 151, 500, 828, 2_000] {
            let plain = Retailer::Kdp.min_margins(pages, false);
            let bleed = Retailer::Kdp.min_margins(pages, true);
            assert!(close(plain.outside_mm, 0.25 * MM_PER_INCH));
            assert!(close(bleed.outside_mm, 0.375 * MM_PER_INCH));
            assert_eq!(plain.inside_mm, bleed.inside_mm);
        }
    }

    #[test]
    fn other_retailers_are_flat() {
        for pages in [0, 100, 500, 1_000] {
            for bleed in [false, true] {
                let ingram = Retailer::IngramSpark.min_margins(pages, bleed);
                assert!(close(ingram.inside_mm, 12.7) && close(ingram.outside_mm, 12.7));
                let bod = Retailer::Bod.min_margins(pages, bleed);
                assert_eq!((bod.inside_mm, bod.outside_mm), (5.0, 5.0));
                let generic = Retailer::Generic.min_margins(pages, bleed);
                assert_eq!((generic.inside_mm, generic.outside_mm), (15.0, 10.0));
            }
        }
    }

    #[test]
    fn margins_never_shrink_as_books_grow() {
        for r in ALL {
            for bleed in [false, true] {
                let mut prev = r.min_margins(0, bleed);
                for pages in 1..=1_000 {
                    let m = r.min_margins(pages, bleed);
                    assert!(m.inside_mm >= prev.inside_mm, "{r:?} {pages}");
                    assert!(m.outside_mm >= prev.outside_mm, "{r:?} {pages}");
                    assert!(m.inside_mm > 0.0 && m.outside_mm > 0.0);
                    prev = m;
                }
            }
        }
    }

    #[test]
    fn margins_fit_every_trim_preset() {
        // Even the thickest KDP book leaves at least half the page width
        // for text on the smallest trim.
        for (name, trim) in TrimSize::presets() {
            for r in ALL {
                let m = r.min_margins(828, true);
                let text = trim.width_mm - m.inside_mm - m.outside_mm;
                assert!(text > trim.width_mm / 2.0, "{name} {r:?}: {text} mm");
            }
        }
    }

    #[test]
    fn ingram_numbers_match_the_file_creation_guide() {
        // 0.5 in (13 mm) on all sides, no gutter scaling, no bleed effect.
        for pages in [18, 200, 600, 1_200] {
            for bleed in [false, true] {
                let m = Retailer::IngramSpark.min_margins(pages, bleed);
                assert!(close(m.inside_mm, 0.5 * MM_PER_INCH));
                assert!(close(m.outside_mm, 0.5 * MM_PER_INCH));
            }
        }
        // 0.125 in (3 mm) on top, bottom and outside; none on the bind edge.
        let bleed = Retailer::IngramSpark.bleed();
        assert!(close(bleed.mm, 3.175) && !bleed.on_gutter);
        // The guide's bleed chart: 6 × 9 in becomes 6.125 × 9.25 in, 5 × 8 in
        // becomes 5.125 × 8.25 in.
        for (trim, w_in, h_in) in [
            (TrimSize::US_6X9, 6.125, 9.25),
            (TrimSize::US_5X8, 5.125, 8.25),
        ] {
            let w = (trim.width_mm + bleed.mm) / MM_PER_INCH;
            let h = (trim.height_mm + 2.0 * bleed.mm) / MM_PER_INCH;
            assert!(close(w, w_in) && close(h, h_in), "{w} × {h}");
        }
        let limits = Retailer::IngramSpark.page_limits().unwrap();
        assert_eq!((limits.min, limits.max), (18, 1_200));
    }

    #[test]
    fn presets_sit_inside_ingram_trim_matrix() {
        // Sizes from IngramSpark's trim size matrix (inches) that Kernsetzer's
        // trim presets correspond to: 5 × 8, 5.25 × 8, 5.5 × 8.5, 5.83 × 8.27
        // (A5), 6 × 9 and 4.72 × 7.48 in (12 × 19 cm). 13.5 × 21.5 cm is not a
        // standard size there; it needs a custom trim (width 4–8.5 in, length
        // 6–11 in on white paper).
        let standard = [
            (5.0, 8.0),
            (5.25, 8.0),
            (5.5, 8.5),
            (5.83, 8.27),
            (6.0, 9.0),
            (4.72, 7.48),
        ];
        for trim in [
            TrimSize::US_5X8,
            TrimSize::US_5_25X8,
            TrimSize::US_5_5X8_5,
            TrimSize::US_6X9,
            TrimSize::A5,
            TrimSize::DE_12X19,
        ] {
            assert!(
                standard.iter().any(|&(w, h)| {
                    (trim.width_mm - w * MM_PER_INCH).abs() < 0.6
                        && (trim.height_mm - h * MM_PER_INCH).abs() < 0.6
                }),
                "{trim:?}"
            );
        }
    }

    #[test]
    fn bod_numbers_match_its_author_pages() {
        // Seven standard, two large, one maxi.
        assert_eq!(BOD_TRIM_SIZES.len(), 10);
        let cm = |t: TrimSize| (t.width_mm / 10.0, t.height_mm / 10.0);
        let listed: Vec<(f32, f32)> = BOD_TRIM_SIZES.iter().map(|(_, t)| cm(*t)).collect();
        for expected in [
            (12.0, 19.0),
            (13.5, 21.5),
            (14.8, 21.0),
            (15.5, 22.0),
            (17.0, 22.0),
            (17.0, 17.0),
            (21.0, 15.0),
            (19.0, 27.0),
            (21.0, 21.0),
            (21.0, 29.7),
        ] {
            assert!(
                listed
                    .iter()
                    .any(|&(w, h)| close(w, expected.0) && close(h, expected.1)),
                "{expected:?}"
            );
        }
        // 5 mm bleed on every side: 12 × 19 cm is delivered as 13 × 20 cm.
        let bleed = Retailer::Bod.bleed();
        assert!(close(bleed.mm, 5.0) && bleed.on_gutter);
        let (w, h) = (
            TrimSize::DE_12X19.width_mm + 2.0 * bleed.mm,
            TrimSize::DE_12X19.height_mm + 2.0 * bleed.mm,
        );
        assert!(close(w, 130.0) && close(h, 200.0), "{w} × {h}");
        // 24 to 900 pages.
        let limits = Retailer::Bod.page_limits().unwrap();
        assert_eq!((limits.min, limits.max), (24, 900));
        assert!(limits.contains(24) && limits.contains(900));
        assert!(!limits.contains(23) && !limits.contains(901));
    }

    #[test]
    fn bod_trim_sizes_are_distinct_and_fit_its_maximum() {
        for (i, (name, trim)) in BOD_TRIM_SIZES.iter().enumerate() {
            assert!(trim.width_mm > 0.0 && trim.height_mm > 0.0, "{name}");
            // The publisher page's maximum format is 21.6 × 29.7 cm.
            assert!(trim.width_mm <= 216.0 && trim.height_mm <= 297.0, "{name}");
            assert!(
                BOD_TRIM_SIZES[i + 1..]
                    .iter()
                    .all(|(n, t)| n != name && t != trim),
                "duplicate {name}"
            );
        }
        // The German formats Kernsetzer already lists are all BoD formats.
        for trim in [TrimSize::DE_12X19, TrimSize::DE_13_5X21_5, TrimSize::A5] {
            assert!(BOD_TRIM_SIZES.iter().any(|(_, t)| *t == trim), "{trim:?}");
        }
    }

    #[test]
    fn page_limits_per_retailer() {
        assert_eq!(
            Retailer::Kdp.page_limits(),
            Some(PageLimits { min: 24, max: 828 })
        );
        assert_eq!(Retailer::Generic.page_limits(), None);
        for r in ALL {
            if let Some(limits) = r.page_limits() {
                assert!(limits.min > 0 && limits.min < limits.max, "{r:?}");
                assert!(limits.min % 2 == 0 && limits.max % 2 == 0, "{r:?}");
            }
        }
    }

    #[test]
    fn bleed_per_retailer() {
        for r in ALL {
            let bleed = r.bleed();
            assert!(bleed.mm > 0.0 && bleed.mm <= 5.0, "{r:?}");
        }
        assert!(close(Retailer::Kdp.bleed().mm, 3.175) && !Retailer::Kdp.bleed().on_gutter);
        assert!(close(Retailer::Generic.bleed().mm, 3.0));
        // Only BoD wants bleed on the binding edge as well.
        let on_gutter: Vec<_> = ALL.iter().filter(|r| r.bleed().on_gutter).collect();
        assert_eq!(on_gutter, [&Retailer::Bod]);
    }

    #[test]
    fn retailer_names_in_json() {
        for (r, name) in [
            (Retailer::Kdp, "kdp"),
            (Retailer::IngramSpark, "ingram-spark"),
            (Retailer::Bod, "bod"),
            (Retailer::Generic, "generic"),
        ] {
            let json = serde_json::to_string(&r).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(serde_json::from_str::<Retailer>(&json).unwrap(), r);
        }
    }

    #[test]
    fn specs_verified_on_is_an_iso_date() {
        let parts: Vec<u32> = SPECS_VERIFIED_ON
            .split('-')
            .map(|p| p.parse().unwrap())
            .collect();
        assert_eq!(SPECS_VERIFIED_ON.len(), 10);
        assert!(parts[0] >= 2026 && (1..=12).contains(&parts[1]) && (1..=31).contains(&parts[2]));
    }

    #[test]
    fn inch_presets_are_kdp_standard_trims() {
        // KDP's US-store trim table (regular and large), in inches, as of
        // SPECS_VERIFIED_ON. A5 appears only in the Japan-store table; 12 × 19
        // and 13.5 × 21.5 cm in neither (custom trim sizes are not described
        // on the pages above), so only the inch presets are checked.
        const KDP_US_TRIMS_IN: [(f32, f32); 16] = [
            (5.0, 8.0),
            (5.06, 7.81),
            (5.25, 8.0),
            (5.5, 8.5),
            (6.0, 9.0),
            (6.14, 9.21),
            (6.69, 9.61),
            (7.0, 10.0),
            (7.44, 9.69),
            (7.5, 9.25),
            (8.0, 10.0),
            (8.25, 6.0),
            (8.25, 8.25),
            (8.27, 11.69),
            (8.5, 8.5),
            (8.5, 11.0),
        ];
        for trim in [
            TrimSize::US_5X8,
            TrimSize::US_5_25X8,
            TrimSize::US_5_5X8_5,
            TrimSize::US_6X9,
        ] {
            assert!(
                KDP_US_TRIMS_IN
                    .iter()
                    .any(|&(w, h)| TrimSize::inches(w, h) == trim),
                "{trim:?}"
            );
        }
    }

    #[test]
    fn trim_presets() {
        let presets = TrimSize::presets();
        for (i, (name, trim)) in presets.iter().enumerate() {
            assert!(
                trim.width_mm > 0.0 && trim.width_mm < trim.height_mm,
                "{name}"
            );
            assert!(
                presets[i + 1..].iter().all(|(n, t)| n != name && t != trim),
                "duplicate preset {name}"
            );
        }
        assert!(close(TrimSize::US_5X8.width_mm, 127.0));
        assert!(close(TrimSize::US_5_5X8_5.height_mm, 215.9));
        assert_eq!(TrimSize::A5, TrimSize::mm(148.0, 210.0));
    }
}
