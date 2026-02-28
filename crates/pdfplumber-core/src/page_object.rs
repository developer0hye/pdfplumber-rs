//! PageObject enum for custom filtering.
//!
//! [`PageObject`] wraps references to different page object types (characters,
//! lines, rectangles, curves, images) so that a single predicate function can
//! inspect any object on a page.

use crate::{Char, Curve, Image, Line, Rect};

/// An enum wrapping references to different page object types.
///
/// Used as the argument to filter predicates, allowing users to match on
/// specific object types and inspect their properties.
///
/// # Example
///
/// ```ignore
/// // Keep only characters with font "Helvetica" and all non-char objects
/// page.filter(|obj| match obj {
///     PageObject::Char(c) => c.fontname == "Helvetica",
///     _ => true,
/// });
/// ```
pub enum PageObject<'a> {
    /// A character object.
    Char(&'a Char),
    /// A line object.
    Line(&'a Line),
    /// A rectangle object.
    Rect(&'a Rect),
    /// A curve object.
    Curve(&'a Curve),
    /// An image object.
    Image(&'a Image),
}
