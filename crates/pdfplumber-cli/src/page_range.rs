/// Parse a page range string like "1,3-5" into a sorted list of 0-indexed page numbers.
///
/// Input is 1-indexed (user-facing). Output is 0-indexed (internal).
/// Returns an error for invalid input (page 0, malformed ranges, etc.).
pub fn parse_page_range(input: &str, page_count: usize) -> Result<Vec<usize>, String> {
    let mut pages = Vec::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start_str, end_str)) = part.split_once('-') {
            let start: usize = start_str
                .trim()
                .parse()
                .map_err(|_| format!("invalid page number: '{start_str}'"))?;
            let end: usize = end_str
                .trim()
                .parse()
                .map_err(|_| format!("invalid page number: '{end_str}'"))?;

            if start == 0 || end == 0 {
                return Err("page 0 is invalid (pages start at 1)".to_string());
            }
            if start > page_count {
                return Err(format!(
                    "page {start} exceeds document page count ({page_count})"
                ));
            }
            if end > page_count {
                return Err(format!(
                    "page {end} exceeds document page count ({page_count})"
                ));
            }

            for p in start..=end {
                pages.push(p - 1); // convert to 0-indexed
            }
        } else {
            let page: usize = part
                .parse()
                .map_err(|_| format!("invalid page number: '{part}'"))?;

            if page == 0 {
                return Err("page 0 is invalid (pages start at 1)".to_string());
            }
            if page > page_count {
                return Err(format!(
                    "page {page} exceeds document page count ({page_count})"
                ));
            }

            pages.push(page - 1);
        }
    }

    pages.sort();
    pages.dedup();
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_page() {
        assert_eq!(parse_page_range("1", 5).unwrap(), vec![0]);
        assert_eq!(parse_page_range("3", 5).unwrap(), vec![2]);
    }

    #[test]
    fn page_range() {
        assert_eq!(parse_page_range("2-4", 5).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn comma_separated() {
        assert_eq!(parse_page_range("1,3,5", 5).unwrap(), vec![0, 2, 4]);
    }

    #[test]
    fn mixed() {
        assert_eq!(
            parse_page_range("1-3,7,10-12", 12).unwrap(),
            vec![0, 1, 2, 6, 9, 10, 11]
        );
    }

    #[test]
    fn page_zero_invalid() {
        let err = parse_page_range("0", 5).unwrap_err();
        assert!(err.contains("invalid"));
    }

    #[test]
    fn page_exceeds_count() {
        let err = parse_page_range("6", 5).unwrap_err();
        assert!(err.contains("exceeds"));
    }

    #[test]
    fn duplicates_removed() {
        assert_eq!(parse_page_range("1,1,2", 5).unwrap(), vec![0, 1]);
    }

    #[test]
    fn whitespace_tolerance() {
        assert_eq!(
            parse_page_range(" 1 , 3 - 5 ", 5).unwrap(),
            vec![0, 2, 3, 4]
        );
    }
}
