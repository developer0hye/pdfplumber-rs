//! Private helpers and PageData implementation for the Page type.

use std::collections::HashMap;

use pdfplumber_core::{Char, Curve, Image, Line, Rect, StructElement};

use crate::cropped_page::PageData;

use super::Page;

/// Recursively collect all structure elements from a tree into a flat list.
pub(super) fn collect_elements(elements: &[StructElement]) -> Vec<&StructElement> {
    let mut result = Vec::new();
    for elem in elements {
        result.push(elem);
        result.extend(collect_elements(&elem.children));
    }
    result
}

/// Walk the structure tree depth-first, collecting chars for each MCID in order.
pub(super) fn collect_chars_by_structure_order<'a>(
    elements: &[StructElement],
    mcid_groups: &HashMap<u32, Vec<&'a Char>>,
    result: &mut Vec<&'a Char>,
    used_mcids: &mut std::collections::HashSet<u32>,
) {
    for elem in elements {
        for &mcid in &elem.mcids {
            if used_mcids.insert(mcid) {
                if let Some(chars) = mcid_groups.get(&mcid) {
                    result.extend(chars);
                }
            }
        }
        collect_chars_by_structure_order(&elem.children, mcid_groups, result, used_mcids);
    }
}

impl PageData for Page {
    fn chars_data(&self) -> &[Char] {
        &self.chars
    }
    fn lines_data(&self) -> &[Line] {
        &self.lines
    }
    fn rects_data(&self) -> &[Rect] {
        &self.rects
    }
    fn curves_data(&self) -> &[Curve] {
        &self.curves
    }
    fn images_data(&self) -> &[Image] {
        &self.images
    }
}
