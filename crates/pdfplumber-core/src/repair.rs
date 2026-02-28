//! PDF repair types for best-effort fixing of common PDF issues.
//!
//! Provides [`RepairOptions`] for configuring which repairs to attempt
//! and [`RepairResult`] for reporting what was fixed.

use std::fmt;

/// Options for controlling which PDF repairs to attempt.
///
/// Each field enables a specific repair strategy. All default to `true`.
#[derive(Debug, Clone)]
pub struct RepairOptions {
    /// Rebuild the cross-reference table by scanning for `obj`/`endobj` markers.
    pub rebuild_xref: bool,
    /// Recalculate stream `/Length` entries from actual stream data.
    pub fix_stream_lengths: bool,
    /// Remove or skip unresolvable object references with warnings.
    pub remove_broken_objects: bool,
}

impl Default for RepairOptions {
    fn default() -> Self {
        Self {
            rebuild_xref: true,
            fix_stream_lengths: true,
            remove_broken_objects: true,
        }
    }
}

impl fmt::Display for RepairOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RepairOptions(rebuild_xref={}, fix_stream_lengths={}, remove_broken_objects={})",
            self.rebuild_xref, self.fix_stream_lengths, self.remove_broken_objects
        )
    }
}

/// Result of a PDF repair operation.
///
/// Contains the log of repairs that were applied and whether the
/// document was modified.
#[derive(Debug, Clone, Default)]
pub struct RepairResult {
    /// Log of repairs applied, one entry per fix.
    pub log: Vec<String>,
}

impl RepairResult {
    /// Create a new empty repair result.
    pub fn new() -> Self {
        Self { log: Vec::new() }
    }

    /// Returns `true` if any repairs were applied.
    pub fn has_repairs(&self) -> bool {
        !self.log.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_options_default_all_enabled() {
        let opts = RepairOptions::default();
        assert!(opts.rebuild_xref);
        assert!(opts.fix_stream_lengths);
        assert!(opts.remove_broken_objects);
    }

    #[test]
    fn repair_options_display() {
        let opts = RepairOptions::default();
        let s = opts.to_string();
        assert!(s.contains("rebuild_xref=true"));
        assert!(s.contains("fix_stream_lengths=true"));
        assert!(s.contains("remove_broken_objects=true"));
    }

    #[test]
    fn repair_options_custom() {
        let opts = RepairOptions {
            rebuild_xref: false,
            fix_stream_lengths: true,
            remove_broken_objects: false,
        };
        assert!(!opts.rebuild_xref);
        assert!(opts.fix_stream_lengths);
        assert!(!opts.remove_broken_objects);
    }

    #[test]
    fn repair_options_clone() {
        let opts1 = RepairOptions::default();
        let opts2 = opts1.clone();
        assert_eq!(opts1.rebuild_xref, opts2.rebuild_xref);
        assert_eq!(opts1.fix_stream_lengths, opts2.fix_stream_lengths);
        assert_eq!(opts1.remove_broken_objects, opts2.remove_broken_objects);
    }

    #[test]
    fn repair_result_new_empty() {
        let result = RepairResult::new();
        assert!(result.log.is_empty());
        assert!(!result.has_repairs());
    }

    #[test]
    fn repair_result_with_entries() {
        let mut result = RepairResult::new();
        result
            .log
            .push("fixed stream length for object 3 0".to_string());
        result
            .log
            .push("removed broken reference to object 5 0".to_string());
        assert!(result.has_repairs());
        assert_eq!(result.log.len(), 2);
    }

    #[test]
    fn repair_result_default() {
        let result = RepairResult::default();
        assert!(!result.has_repairs());
    }
}
