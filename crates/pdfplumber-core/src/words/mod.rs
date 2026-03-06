use crate::bidi::is_arabic_diacritic_text;
use crate::geometry::BBox;
use crate::text::{Char, TextDirection};

/// Options for word extraction, matching pdfplumber defaults.
#[derive(Debug, Clone)]
pub struct WordOptions {
    /// Maximum horizontal distance between characters to group into a word.
    pub x_tolerance: f64,
    /// Maximum vertical distance between characters to group into a word.
    pub y_tolerance: f64,
    /// If true, include blank/space characters in words instead of splitting on them.
    pub keep_blank_chars: bool,
    /// If true, use the text flow order from the PDF content stream instead of spatial ordering.
    pub use_text_flow: bool,
    /// Text direction for grouping characters.
    pub text_direction: TextDirection,
    /// If true, expand common Latin ligatures (U+FB00–U+FB06) to their multi-character equivalents.
    pub expand_ligatures: bool,
}

impl Default for WordOptions {
    fn default() -> Self {
        Self {
            x_tolerance: 3.0,
            y_tolerance: 3.0,
            keep_blank_chars: false,
            use_text_flow: false,
            text_direction: TextDirection::default(),
            expand_ligatures: true,
        }
    }
}

/// A word extracted from a PDF page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Word {
    /// The text content of this word.
    pub text: String,
    /// Bounding box encompassing all constituent characters.
    pub bbox: BBox,
    /// Distance from the top of the first page (minimum doctop of constituent chars).
    pub doctop: f64,
    /// Text direction for this word.
    pub direction: TextDirection,
    /// The characters that make up this word.
    pub chars: Vec<Char>,
}

/// Extracts words from a sequence of characters based on spatial proximity.
pub struct WordExtractor;

impl WordExtractor {
    /// Extract words from the given characters using the specified options.
    ///
    /// Characters are grouped into words based on spatial proximity:
    /// - Characters within `x_tolerance` horizontally and `y_tolerance` vertically
    ///   are grouped together.
    /// - For CJK characters, character width (or height for vertical text) is used
    ///   as the tolerance instead of the fixed `x_tolerance`/`y_tolerance`.
    /// - By default, whitespace characters split words. Set `keep_blank_chars`
    ///   to include them.
    /// - By default, characters are sorted spatially. Set `use_text_flow` to
    ///   preserve PDF content stream order.
    /// - `text_direction` controls sorting and gap logic for vertical text.
    ///
    /// Horizontal chars (Ltr + Rtl) are merged and sorted spatially left-to-right,
    /// matching Python pdfplumber behavior. Only vertical chars (Ttb/Btt) are
    /// partitioned into separate groups with vertical sorting.
    pub fn extract(chars: &[Char], options: &WordOptions) -> Vec<Word> {
        if chars.is_empty() {
            return Vec::new();
        }

        // Check if any chars have vertical per-char direction (Ttb/Btt).
        // Horizontal chars (Ltr + Rtl) are always merged and sorted spatially,
        // matching Python pdfplumber which sorts all upright chars left-to-right.
        // Vertical chars (Ttb + Btt) are merged and sorted top-to-bottom,
        // matching Python pdfplumber which sorts all non-upright chars by top.
        let has_vertical = chars
            .iter()
            .any(|c| matches!(c.direction, TextDirection::Ttb | TextDirection::Btt));

        if !has_vertical {
            // All chars are horizontal (Ltr or Rtl) → spatial LTR sorting
            return Self::extract_group(chars, options, None);
        }

        // Partition into horizontal (Ltr + Rtl) and vertical (Ttb + Btt) groups.
        let mut horizontal_chars: Vec<Char> = Vec::new();
        let mut vertical_chars: Vec<Char> = Vec::new();
        for ch in chars {
            match ch.direction {
                TextDirection::Ltr | TextDirection::Rtl => horizontal_chars.push(ch.clone()),
                TextDirection::Ttb | TextDirection::Btt => vertical_chars.push(ch.clone()),
            }
        }

        let mut words = Vec::new();
        if !horizontal_chars.is_empty() {
            words.extend(Self::extract_group(&horizontal_chars, options, None));
        }
        if !vertical_chars.is_empty() {
            // All vertical chars use TTB sorting (spatial top-to-bottom),
            // matching Python pdfplumber behavior.
            words.extend(Self::extract_group(
                &vertical_chars,
                options,
                Some(TextDirection::Ttb),
            ));
        }
        words
    }

    /// Extract words from a group of chars that share the same orientation.
    ///
    /// When `force_direction` is `Some`, the specified direction overrides
    /// `options.text_direction` for sorting and splitting. This enables
    /// per-char direction handling where different groups of chars on the
    /// same page use different sorting logic.
    fn extract_group(
        chars: &[Char],
        options: &WordOptions,
        force_direction: Option<TextDirection>,
    ) -> Vec<Word> {
        if chars.is_empty() {
            return Vec::new();
        }

        let effective_direction = force_direction.unwrap_or(options.text_direction);

        let mut sorted_chars: Vec<&Char> = chars.iter().collect();
        if !options.use_text_flow {
            let sort_opts = if force_direction.is_some() {
                WordOptions {
                    text_direction: effective_direction,
                    ..options.clone()
                }
            } else {
                options.clone()
            };
            Self::cluster_sort(&mut sorted_chars, &sort_opts);
        }

        let is_vertical = matches!(effective_direction, TextDirection::Ttb | TextDirection::Btt);

        let mut words = Vec::new();
        let mut current_chars: Vec<Char> = Vec::new();

        for &ch in &sorted_chars {
            let is_blank = ch.text.chars().all(|c| c.is_whitespace());

            // If this is a blank and we're not keeping blanks, finish current word
            if is_blank && !options.keep_blank_chars {
                if !current_chars.is_empty() {
                    words.push(Self::make_word(&current_chars, options.expand_ligatures));
                    current_chars.clear();
                }
                continue;
            }

            if current_chars.is_empty() {
                current_chars.push(ch.clone());
                continue;
            }

            let last = current_chars.last().unwrap();

            let should_split = if is_vertical {
                Self::should_split_vertical(last, ch, options)
            } else {
                Self::should_split_horizontal(last, ch, options)
            };

            if should_split {
                words.push(Self::make_word(&current_chars, options.expand_ligatures));
                current_chars.clear();
            }

            current_chars.push(ch.clone());
        }

        if !current_chars.is_empty() {
            words.push(Self::make_word(&current_chars, options.expand_ligatures));
        }

        words
    }

    /// Sort chars by clustering on the cross-direction coordinate (within
    /// tolerance), then sorting within each cluster by reading direction.
    ///
    /// This matches Python pdfplumber's `cluster_objects` approach: chars are
    /// first sorted by the cross-direction (e.g., `top` for horizontal text),
    /// then consecutive chars within `tolerance` are grouped into the same
    /// cluster ("line"). Within each cluster, chars are sorted by reading
    /// direction (e.g., `x0` for LTR).
    fn cluster_sort(chars: &mut Vec<&Char>, options: &WordOptions) {
        let is_vertical = matches!(
            options.text_direction,
            TextDirection::Ttb | TextDirection::Btt
        );

        // Step 1: Sort by cross-direction coordinate
        if is_vertical {
            // Vertical text: columns go right-to-left, so sort by x0 descending
            chars.sort_by(|a, b| {
                b.bbox
                    .x0
                    .partial_cmp(&a.bbox.x0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            // Horizontal text: lines go top-to-bottom, so sort by top ascending
            chars.sort_by(|a, b| {
                a.bbox
                    .top
                    .partial_cmp(&b.bbox.top)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // Step 2: Cluster consecutive chars within cross-direction tolerance
        // Step 3: Sort within each cluster by reading-direction coordinate
        let cross_tolerance = if is_vertical {
            options.x_tolerance
        } else {
            options.y_tolerance
        };

        // Find cluster boundaries
        let mut cluster_starts: Vec<usize> = vec![0];
        for i in 1..chars.len() {
            let cross_diff = if is_vertical {
                (chars[i - 1].bbox.x0 - chars[i].bbox.x0).abs()
            } else {
                (chars[i].bbox.top - chars[i - 1].bbox.top).abs()
            };
            if cross_diff > cross_tolerance {
                cluster_starts.push(i);
            }
        }
        cluster_starts.push(chars.len());

        // Sort within each cluster by reading-direction
        for window in cluster_starts.windows(2) {
            let (start, end) = (window[0], window[1]);
            let cluster = &mut chars[start..end];
            match options.text_direction {
                TextDirection::Ttb => {
                    cluster.sort_by(|a, b| {
                        a.bbox
                            .top
                            .partial_cmp(&b.bbox.top)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                TextDirection::Btt => {
                    cluster.sort_by(|a, b| {
                        b.bbox
                            .bottom
                            .partial_cmp(&a.bbox.bottom)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                TextDirection::Rtl => {
                    // Detect physical layout direction within the cluster.
                    // BiDi RTL: chars are physically left-to-right (ascending x0)
                    //   but have Unicode RTL direction → sort ascending for visual order.
                    // Physical RTL: chars are physically right-to-left (descending x0)
                    //   from TRM with negative a → sort descending for reading order.
                    let is_physically_ltr = if cluster.len() >= 2 {
                        let ascending_pairs = cluster
                            .windows(2)
                            .filter(|w| w[1].bbox.x0 >= w[0].bbox.x0)
                            .count();
                        ascending_pairs >= cluster.len() / 2
                    } else {
                        true
                    };

                    if is_physically_ltr {
                        cluster.sort_by(|a, b| {
                            a.bbox
                                .x0
                                .partial_cmp(&b.bbox.x0)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    } else {
                        cluster.sort_by(|a, b| {
                            b.bbox
                                .x0
                                .partial_cmp(&a.bbox.x0)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                }
                _ => {
                    // LTR (default)
                    cluster.sort_by(|a, b| {
                        a.bbox
                            .x0
                            .partial_cmp(&b.bbox.x0)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }
        }
    }

    /// Check if two horizontally-adjacent chars should be split into separate words.
    ///
    /// Uses direction-agnostic gap: the geometric distance between x-intervals.
    /// Returns 0 for overlapping/touching chars and positive for separated chars.
    /// This matches Python pdfplumber behavior where overlapping chars (e.g.,
    /// duplicate chars for bold rendering) are always grouped together.
    ///
    /// Arabic diacritical marks (harakat/tashkil) are never split from their
    /// preceding base character, since they are rendered above/below and may
    /// have slight positional gaps in the PDF.
    ///
    /// Uses flat `x_tolerance` / `y_tolerance` for all chars (matching Python
    /// pdfplumber — no CJK-specific tolerance expansion).
    fn should_split_horizontal(last: &Char, current: &Char, options: &WordOptions) -> bool {
        // Arabic diacritical marks always combine with their base character.
        // Check both directions: a diacritic following a base, or a base
        // following a diacritic (when sorted by y-position, diacritics above
        // the baseline may appear before their base character).
        if is_arabic_diacritic_text(&current.text) || is_arabic_diacritic_text(&last.text) {
            return false;
        }

        let x_gap =
            (last.bbox.x0.max(current.bbox.x0) - last.bbox.x1.min(current.bbox.x1)).max(0.0);
        let y_diff = (current.bbox.top - last.bbox.top).abs();
        x_gap > options.x_tolerance || y_diff > options.y_tolerance
    }

    /// Check if two vertically-adjacent chars should be split into separate words.
    ///
    /// Uses direction-agnostic gap: the geometric distance between y-intervals.
    /// Handles both TTB and BTT text correctly.
    fn should_split_vertical(last: &Char, current: &Char, options: &WordOptions) -> bool {
        let y_gap = (last.bbox.top.max(current.bbox.top)
            - last.bbox.bottom.min(current.bbox.bottom))
        .max(0.0);
        let x_diff = (current.bbox.x0 - last.bbox.x0).abs();
        y_gap > options.y_tolerance || x_diff > options.x_tolerance
    }

    fn make_word(chars: &[Char], expand_ligatures: bool) -> Word {
        let raw_text: String = chars.iter().map(|c| c.text.as_str()).collect();
        let text = if expand_ligatures {
            expand_ligatures_in_text(&raw_text)
        } else {
            raw_text
        };
        let bbox = chars
            .iter()
            .map(|c| c.bbox)
            .reduce(|a, b| a.union(&b))
            .expect("make_word called with non-empty chars");
        let doctop = chars.iter().map(|c| c.doctop).fold(f64::INFINITY, f64::min);
        let direction = chars[0].direction;
        Word {
            text,
            bbox,
            doctop,
            direction,
            chars: chars.to_vec(),
        }
    }
}

/// Expand common Latin ligatures (U+FB00–U+FB06) to their multi-character equivalents.
fn expand_ligatures_in_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{FB00}' => result.push_str("ff"),
            '\u{FB01}' => result.push_str("fi"),
            '\u{FB02}' => result.push_str("fl"),
            '\u{FB03}' => result.push_str("ffi"),
            '\u{FB04}' => result.push_str("ffl"),
            '\u{FB05}' => result.push_str("\u{017F}t"), // long s + t
            '\u{FB06}' => result.push_str("st"),
            _ => result.push(ch),
        }
    }
    result
}

#[cfg(test)]
mod tests;
