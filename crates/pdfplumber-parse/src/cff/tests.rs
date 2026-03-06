#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // ========== Test helpers ==========

    /// Public test helper for building CFF data from other test modules.
    pub fn build_cff_data_for_test(
        default_width_x: i32,
        nominal_width_x: i32,
        glyph_widths: &[i32],
    ) -> Vec<u8> {
        build_cff_data(default_width_x, nominal_width_x, glyph_widths)
    }

    /// Encode a CFF integer operand in DICT format.
    fn encode_dict_int(val: i32) -> Vec<u8> {
        if (-107..=107).contains(&val) {
            vec![(val + 139) as u8]
        } else if (108..=1131).contains(&val) {
            let adjusted = val - 108;
            let b0 = (adjusted / 256 + 247) as u8;
            let b1 = (adjusted % 256) as u8;
            vec![b0, b1]
        } else if (-1131..=-108).contains(&val) {
            let adjusted = -val - 108;
            let b0 = (adjusted / 256 + 251) as u8;
            let b1 = (adjusted % 256) as u8;
            vec![b0, b1]
        } else if (-32768..=32767).contains(&val) {
            let bytes = (val as i16).to_be_bytes();
            vec![28, bytes[0], bytes[1]]
        } else {
            let bytes = val.to_be_bytes();
            vec![29, bytes[0], bytes[1], bytes[2], bytes[3]]
        }
    }

    /// Build a CFF INDEX from a list of byte arrays.
    fn build_index(items: &[&[u8]]) -> Vec<u8> {
        let count = items.len() as u16;
        if count == 0 {
            return vec![0, 0]; // empty INDEX
        }

        // Calculate offsets (1-based)
        let mut offsets: Vec<usize> = vec![1];
        for item in items {
            offsets.push(offsets.last().unwrap() + item.len());
        }

        // Determine offSize (use 1, 2, or 4 bytes)
        let max_offset = *offsets.last().unwrap();
        let off_size: u8 = if max_offset <= 255 {
            1
        } else if max_offset <= 65535 {
            2
        } else {
            4
        };

        let mut buf = Vec::new();
        // count (2 bytes)
        buf.extend_from_slice(&count.to_be_bytes());
        // offSize (1 byte)
        buf.push(off_size);
        // offset array
        for &off in &offsets {
            match off_size {
                1 => buf.push(off as u8),
                2 => buf.extend_from_slice(&(off as u16).to_be_bytes()),
                4 => buf.extend_from_slice(&(off as u32).to_be_bytes()),
                _ => unreachable!(),
            }
        }
        // data
        for item in items {
            buf.extend_from_slice(item);
        }
        buf
    }

    /// Build a minimal CFF font with specified widths.
    ///
    /// Creates a CFF with:
    /// - One font (in Name INDEX)
    /// - Top DICT pointing to CharStrings and Private
    /// - Private DICT with defaultWidthX and nominalWidthX
    /// - CharStrings with simple charstrings encoding the widths
    fn build_cff_data(
        default_width_x: i32,
        nominal_width_x: i32,
        glyph_widths: &[i32], // actual widths in 1/1000
    ) -> Vec<u8> {
        // Build CharStrings: each glyph is either:
        //   - endchar (if width == default_width_x)
        //   - (width - nominal_width_x) endchar (if width != default_width_x)
        let mut charstrings: Vec<Vec<u8>> = Vec::new();
        for &w in glyph_widths {
            let mut cs = Vec::new();
            if w == default_width_x {
                // No width operand, just endchar
                cs.push(14); // endchar
            } else {
                // Width operand + endchar
                let width_val = w - nominal_width_x;
                cs.extend_from_slice(&encode_dict_int(width_val));
                cs.push(14); // endchar
            }
            charstrings.push(cs);
        }

        // Build Private DICT
        let mut private_dict = Vec::new();
        if default_width_x != 0 {
            private_dict.extend_from_slice(&encode_dict_int(default_width_x));
            private_dict.push(20); // defaultWidthX operator
        }
        if nominal_width_x != 0 {
            private_dict.extend_from_slice(&encode_dict_int(nominal_width_x));
            private_dict.push(21); // nominalWidthX operator
        }
        if private_dict.is_empty() {
            // Need at least something; put a dummy
            private_dict.extend_from_slice(&encode_dict_int(0));
            private_dict.push(20);
        }

        // Build CharStrings INDEX
        let cs_refs: Vec<&[u8]> = charstrings.iter().map(|cs| cs.as_slice()).collect();
        let charstrings_index = build_index(&cs_refs);

        // We need to know offsets, so build in stages:
        // CFF structure:
        //   Header (4 bytes)
        //   Name INDEX
        //   Top DICT INDEX
        //   String INDEX (empty)
        //   Global Subr INDEX (empty)
        //   CharStrings INDEX
        //   Private DICT

        let header = vec![1u8, 0, 4, 1]; // major=1, minor=0, hdrSize=4, offSize=1

        let name_index = build_index(&[b"TestFont"]);

        let string_index = build_index(&[]); // empty
        let global_subr_index = build_index(&[]); // empty

        // Calculate offsets for CharStrings and Private
        // First pass: build Top DICT with placeholder offsets, measure sizes
        // We'll use a two-pass approach:

        // Phase 1: compute sizes without Top DICT content
        let header_size = header.len();
        let name_size = name_index.len();

        // We need to know Top DICT INDEX size to compute subsequent offsets.
        // Build a dummy Top DICT first, then adjust.
        // Top DICT needs: charstrings_offset (op 17), private size+offset (op 18)

        // Estimate: Top DICT will have at most ~20 bytes
        // Let's build it properly with a two-pass approach.

        // For simplicity: manually compute offsets.
        // After header + name + top_dict + string + global_subr, charstrings start.
        // Then private DICT follows charstrings.

        // Build top_dict content (will need to adjust offsets)
        let build_top_dict = |cs_offset: i32, priv_size: i32, priv_offset: i32| -> Vec<u8> {
            let mut td = Vec::new();
            // CharStrings offset
            td.extend_from_slice(&encode_dict_int(cs_offset));
            td.push(17);
            // Private: size offset
            td.extend_from_slice(&encode_dict_int(priv_size));
            td.extend_from_slice(&encode_dict_int(priv_offset));
            td.push(18);
            td
        };

        // First estimate: use large placeholder values to determine Top DICT INDEX size
        let td_est = build_top_dict(9999, private_dict.len() as i32, 99999);
        let td_refs_est: Vec<&[u8]> = vec![td_est.as_slice()];
        let top_dict_index_est = build_index(&td_refs_est);

        let cs_offset_est = header_size
            + name_size
            + top_dict_index_est.len()
            + string_index.len()
            + global_subr_index.len();

        let priv_offset_est = cs_offset_est + charstrings_index.len();

        // Now build with real values
        let td = build_top_dict(
            cs_offset_est as i32,
            private_dict.len() as i32,
            priv_offset_est as i32,
        );
        let td_refs: Vec<&[u8]> = vec![td.as_slice()];
        let top_dict_index = build_index(&td_refs);

        // Verify our size estimate was correct
        let actual_cs_offset = header_size
            + name_size
            + top_dict_index.len()
            + string_index.len()
            + global_subr_index.len();

        let actual_priv_offset = actual_cs_offset + charstrings_index.len();

        // If sizes differ from estimate, rebuild (should rarely happen)
        let (top_dict_index, charstrings_offset_final, private_offset_final) = if actual_cs_offset
            != cs_offset_est
            || actual_priv_offset != priv_offset_est
        {
            let td2 = build_top_dict(
                actual_cs_offset as i32,
                private_dict.len() as i32,
                actual_priv_offset as i32,
            );
            let td2_refs: Vec<&[u8]> = vec![td2.as_slice()];
            let tdi2 = build_index(&td2_refs);
            let cs2 =
                header_size + name_size + tdi2.len() + string_index.len() + global_subr_index.len();
            let pr2 = cs2 + charstrings_index.len();
            (tdi2, cs2, pr2)
        } else {
            (top_dict_index, actual_cs_offset, actual_priv_offset)
        };

        // If sizes still differ after rebuild, do one more iteration
        let (top_dict_index, _cs_off, _priv_off) = {
            let verify_cs = header_size
                + name_size
                + top_dict_index.len()
                + string_index.len()
                + global_subr_index.len();
            let verify_priv = verify_cs + charstrings_index.len();
            if verify_cs != charstrings_offset_final || verify_priv != private_offset_final {
                let td3 = build_top_dict(
                    verify_cs as i32,
                    private_dict.len() as i32,
                    verify_priv as i32,
                );
                let td3_refs: Vec<&[u8]> = vec![td3.as_slice()];
                let tdi3 = build_index(&td3_refs);
                let cs3 = header_size
                    + name_size
                    + tdi3.len()
                    + string_index.len()
                    + global_subr_index.len();
                let pr3 = cs3 + charstrings_index.len();
                (tdi3, cs3, pr3)
            } else {
                (
                    top_dict_index,
                    charstrings_offset_final,
                    private_offset_final,
                )
            }
        };

        // Assemble
        let mut buf = Vec::new();
        buf.extend_from_slice(&header);
        buf.extend_from_slice(&name_index);
        buf.extend_from_slice(&top_dict_index);
        buf.extend_from_slice(&string_index);
        buf.extend_from_slice(&global_subr_index);
        buf.extend_from_slice(&charstrings_index);
        buf.extend_from_slice(&private_dict);

        buf
    }

    // ========== TDD: Unit tests for CFF INDEX parsing ==========

    #[test]
    fn parse_empty_index() {
        let data = build_index(&[]);
        let (items, end) = parse_index(&data, 0).expect("should parse empty INDEX");
        assert!(items.is_empty());
        assert_eq!(end, 2); // count(2) only
    }

    #[test]
    fn parse_single_item_index() {
        let data = build_index(&[b"hello"]);
        let (items, _) = parse_index(&data, 0).expect("should parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], b"hello");
    }

    #[test]
    fn parse_multi_item_index() {
        let data = build_index(&[b"abc", b"de", b"f"]);
        let (items, _) = parse_index(&data, 0).expect("should parse");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], b"abc");
        assert_eq!(items[1], b"de");
        assert_eq!(items[2], b"f");
    }

    #[test]
    fn skip_index_correct_end() {
        let data = build_index(&[b"test", b"data"]);
        let end_skip = skip_index(&data, 0).expect("should skip");
        let (_, end_parse) = parse_index(&data, 0).expect("should parse");
        assert_eq!(end_skip, end_parse);
    }

    // ========== TDD: Unit tests for DICT number encoding ==========

    #[test]
    fn encode_dict_int_small_positive() {
        assert_eq!(encode_dict_int(0), vec![139]);
        assert_eq!(encode_dict_int(100), vec![239]);
        assert_eq!(encode_dict_int(107), vec![246]);
    }

    #[test]
    fn encode_dict_int_small_negative() {
        assert_eq!(encode_dict_int(-1), vec![138]);
        assert_eq!(encode_dict_int(-107), vec![32]);
    }

    #[test]
    fn encode_dict_int_medium_positive() {
        // 108..=1131
        let enc = encode_dict_int(108);
        assert_eq!(enc.len(), 2);
        assert_eq!(enc[0], 247);
    }

    #[test]
    fn encode_dict_int_medium_negative() {
        let enc = encode_dict_int(-108);
        assert_eq!(enc.len(), 2);
        assert_eq!(enc[0], 251);
    }

    #[test]
    fn encode_dict_int_large() {
        let enc = encode_dict_int(5000);
        assert_eq!(enc[0], 28); // 3-byte format
        assert_eq!(enc.len(), 3);
    }

    #[test]
    fn encode_dict_int_very_large() {
        let enc = encode_dict_int(100000);
        assert_eq!(enc[0], 29); // 5-byte format
        assert_eq!(enc.len(), 5);
    }

    // ========== TDD: Unit tests for real number parsing ==========

    #[test]
    fn parse_real_simple() {
        // Encode 3.14: nibbles 3, '.', 1, 4, 0xF
        // byte 1: 0x3A (3, '.')
        // byte 2: 0x14 (1, 4)
        // byte 3: 0xFF (terminator)
        let data = [30u8, 0x3A, 0x14, 0xFF];
        let (val, consumed) = parse_real_number(&data, 0).expect("should parse");
        assert!((val - 3.14).abs() < 0.001);
        assert_eq!(consumed, 4);
    }

    #[test]
    fn parse_real_negative() {
        // Encode -2.5: nibbles '-', 2, '.', 5, 0xF
        // 0xE = '-', 0x2 = 2, 0xA = '.', 0x5 = 5
        // byte 1: 0xE2 (-, 2)
        // byte 2: 0xA5 (., 5)
        // byte 3: 0xFF (terminator)
        let data = [30u8, 0xE2, 0xA5, 0xFF];
        let (val, consumed) = parse_real_number(&data, 0).expect("should parse");
        assert!((val - (-2.5)).abs() < 0.001);
        assert_eq!(consumed, 4);
    }

    // ========== TDD: Unit tests for CharString width extraction ==========

    #[test]
    fn charstring_width_endchar_only() {
        // endchar with no args → default width
        let data = [14u8]; // endchar
        let w = extract_charstring_width(&data, 500.0, 0.0);
        assert!((w - 500.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_with_explicit_width_endchar() {
        // width=100, endchar → nominal + 100
        let mut cs = encode_dict_int(100);
        cs.push(14); // endchar
        let w = extract_charstring_width(&cs, 500.0, 200.0);
        assert!((w - 300.0).abs() < 0.01); // nominal(200) + 100
    }

    #[test]
    fn charstring_width_hmoveto_no_width() {
        // hmoveto with 1 arg (dx) → default width
        let mut cs = encode_dict_int(50); // dx
        cs.push(22); // hmoveto
        let w = extract_charstring_width(&cs, 600.0, 0.0);
        assert!((w - 600.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_hmoveto_with_width() {
        // width + dx, hmoveto → nominal + width
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(300)); // width
        cs.extend_from_slice(&encode_dict_int(50)); // dx
        cs.push(22); // hmoveto
        let w = extract_charstring_width(&cs, 0.0, 200.0);
        assert!((w - 500.0).abs() < 0.01); // nominal(200) + 300
    }

    #[test]
    fn charstring_width_vmoveto_with_width() {
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(-100)); // width
        cs.extend_from_slice(&encode_dict_int(50)); // dy
        cs.push(4); // vmoveto
        let w = extract_charstring_width(&cs, 0.0, 500.0);
        assert!((w - 400.0).abs() < 0.01); // nominal(500) + (-100)
    }

    #[test]
    fn charstring_width_rmoveto_no_width() {
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(10)); // dx
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(21); // rmoveto
        let w = extract_charstring_width(&cs, 700.0, 0.0);
        assert!((w - 700.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_rmoveto_with_width() {
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(50)); // width
        cs.extend_from_slice(&encode_dict_int(10)); // dx
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(21); // rmoveto
        let w = extract_charstring_width(&cs, 0.0, 100.0);
        assert!((w - 150.0).abs() < 0.01); // nominal(100) + 50
    }

    #[test]
    fn charstring_width_hstem_no_width() {
        // hstem with 2 args (pair) → default width
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(10)); // y
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(1); // hstem
        let w = extract_charstring_width(&cs, 400.0, 0.0);
        assert!((w - 400.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_hstem_with_width() {
        // hstem with 3 args (width + pair) → nominal + width
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(200)); // width
        cs.extend_from_slice(&encode_dict_int(10)); // y
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(1); // hstem
        let w = extract_charstring_width(&cs, 0.0, 300.0);
        assert!((w - 500.0).abs() < 0.01); // nominal(300) + 200
    }

    // ========== TDD: Full CFF parsing integration tests ==========

    #[test]
    fn parse_cff_basic_widths() {
        // 3 glyphs with widths 250, 500, 750 (default=0, nominal=0)
        let data = build_cff_data(0, 0, &[250, 500, 750]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 3);
        assert!((widths.get_width(0).unwrap() - 250.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 500.0).abs() < 0.01);
        assert!((widths.get_width(2).unwrap() - 750.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_with_default_width() {
        // Glyphs with default width (all same width = defaultWidthX)
        let data = build_cff_data(600, 0, &[600, 600, 600]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 3);
        for gid in 0..3 {
            assert!(
                (widths.get_width(gid).unwrap() - 600.0).abs() < 0.01,
                "glyph {} width mismatch",
                gid
            );
        }
    }

    #[test]
    fn parse_cff_with_nominal_width() {
        // nominal=200, widths: 300 (encoded as 100), 500 (encoded as 300)
        let data = build_cff_data(0, 200, &[300, 500]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 2);
        assert!((widths.get_width(0).unwrap() - 300.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 500.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_mixed_default_and_explicit() {
        // default=500, nominal=400
        // Glyph 0: width=500 (default, no operand)
        // Glyph 1: width=600 (encoded as 600-400=200)
        let data = build_cff_data(500, 400, &[500, 600]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 2);
        assert!((widths.get_width(0).unwrap() - 500.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 600.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_single_glyph() {
        let data = build_cff_data(0, 0, &[800]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");
        assert_eq!(widths.num_glyphs(), 1);
        assert!((widths.get_width(0).unwrap() - 800.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_out_of_range_glyph() {
        let data = build_cff_data(0, 0, &[500, 600]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");
        assert!(widths.get_width(2).is_none());
        assert!(widths.get_width(100).is_none());
    }

    #[test]
    fn parse_cff_empty_data() {
        assert!(parse_cff_widths(&[]).is_none());
    }

    #[test]
    fn parse_cff_truncated_header() {
        assert!(parse_cff_widths(&[1, 0]).is_none());
    }

    #[test]
    fn parse_cff_invalid_version() {
        // Version 2 (not supported)
        let mut data = build_cff_data(0, 0, &[500]);
        data[0] = 2; // major version 2
        assert!(parse_cff_widths(&data).is_none());
    }

    #[test]
    fn parse_cff_negative_width() {
        // Some fonts have negative widths (rare but valid)
        let data = build_cff_data(0, 0, &[-100]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");
        assert!((widths.get_width(0).unwrap() - (-100.0)).abs() < 0.01);
    }
}
