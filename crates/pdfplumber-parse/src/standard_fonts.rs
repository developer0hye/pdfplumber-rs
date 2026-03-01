//! Standard 14 Type1 font width tables.
//!
//! Provides built-in glyph width data (in 1/1000 em-square units) for the
//! 14 standard PDF Type1 fonts. These widths are used as a fallback when
//! a PDF font dictionary lacks an explicit /Widths array.
//!
//! Width data is sourced from Adobe AFM (Adobe Font Metrics) specifications
//! and indexed by WinAnsiEncoding character codes.

/// Font metrics data for a standard Type1 font.
#[derive(Debug, Clone)]
pub struct StandardFontData {
    /// Glyph widths indexed by character code (0-255), in 1/1000 em-square units.
    pub widths: [u16; 256],
    /// Font bounding box [llx, lly, urx, ury] in 1/1000 em-square units.
    pub font_bbox: [i16; 4],
}

/// Look up standard font data by font name.
///
/// Returns `Some` for any of the 14 standard Type1 font names:
/// Courier (4 variants), Helvetica (4 variants), Times (4 variants),
/// Symbol, ZapfDingbats.
///
/// Returns `None` for unknown font names.
pub fn lookup(name: &str) -> Option<&'static StandardFontData> {
    match name {
        "Courier" | "Courier-Bold" | "Courier-Oblique" | "Courier-BoldOblique" => Some(&COURIER),
        "Helvetica" | "Helvetica-Oblique" => Some(&HELVETICA),
        "Helvetica-Bold" | "Helvetica-BoldOblique" => Some(&HELVETICA_BOLD),
        "Times-Roman" => Some(&TIMES_ROMAN),
        "Times-Bold" => Some(&TIMES_BOLD),
        "Times-Italic" => Some(&TIMES_ITALIC),
        "Times-BoldItalic" => Some(&TIMES_BOLD_ITALIC),
        "Symbol" => Some(&SYMBOL),
        "ZapfDingbats" => Some(&ZAPF_DINGBATS),
        _ => None,
    }
}

// =============================================================================
// Courier — monospaced, all widths 600
// =============================================================================
static COURIER: StandardFontData = StandardFontData {
    widths: [600; 256],
    font_bbox: [-23, -250, 715, 805],
};

// =============================================================================
// Helvetica (also used for Helvetica-Oblique)
// Width data from Adobe Helvetica AFM, mapped via WinAnsiEncoding.
// =============================================================================
#[rustfmt::skip]
static HELVETICA: StandardFontData = StandardFontData {
    widths: [
        // 0-15: control characters
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 16-31: control characters
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47: space ! " # $ % & ' ( ) * + , - . /
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
        // 48-63: 0 1 2 3 4 5 6 7 8 9 : ; < = > ?
        556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
        // 64-79: @ A B C D E F G H I J K L M N O
        1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
        // 80-95: P Q R S T U V W X Y Z [ \ ] ^ _
        667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
        // 96-111: ` a b c d e f g h i j k l m n o
        333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
        // 112-127: p q r s t u v w x y z { | } ~ DEL
        556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, 0,
        // 128-143: Euro . quotesinglbase florin quotedblbase ellipsis dagger daggerdbl
        //          circumflex perthousand Scaron guilsinglleft OE . Zcaron .
        556, 0, 222, 556, 333, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0,
        // 144-159: . quoteleft quoteright quotedblleft quotedblright bullet endash emdash
        //          tilde trademark scaron guilsinglright oe . zcaron Ydieresis
        0, 222, 222, 333, 333, 350, 556, 1000, 333, 1000, 500, 333, 944, 0, 500, 667,
        // 160-175: nbspace exclamdown cent sterling currency yen brokenbar section
        //          dieresis copyright ordfeminine guillemotleft logicalnot softhyphen registered macron
        278, 333, 556, 556, 556, 556, 260, 556, 333, 737, 370, 556, 584, 333, 737, 333,
        // 176-191: degree plusminus twosuperior threesuperior acute mu paragraph periodcentered
        //          cedilla onesuperior ordmasculine guillemotright onequarter onehalf threequarters questiondown
        400, 584, 333, 333, 333, 556, 537, 278, 333, 333, 365, 556, 834, 834, 834, 611,
        // 192-207: Agrave Aacute Acircumflex Atilde Adieresis Aring AE Ccedilla
        //          Egrave Eacute Ecircumflex Edieresis Igrave Iacute Icircumflex Idieresis
        667, 667, 667, 667, 667, 667, 1000, 722, 667, 667, 667, 667, 278, 278, 278, 278,
        // 208-223: Eth Ntilde Ograve Oacute Ocircumflex Otilde Odieresis multiply
        //          Oslash Ugrave Uacute Ucircumflex Udieresis Yacute Thorn germandbls
        722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667, 611,
        // 224-239: agrave aacute acircumflex atilde adieresis aring ae ccedilla
        //          egrave eacute ecircumflex edieresis igrave iacute icircumflex idieresis
        556, 556, 556, 556, 556, 556, 889, 500, 556, 556, 556, 556, 278, 278, 278, 278,
        // 240-255: eth ntilde ograve oacute ocircumflex otilde odieresis divide
        //          oslash ugrave uacute ucircumflex udieresis yacute thorn ydieresis
        556, 556, 556, 556, 556, 556, 556, 584, 611, 556, 556, 556, 556, 500, 556, 500,
    ],
    font_bbox: [-166, -225, 1000, 931],
};

// =============================================================================
// Helvetica-Bold (also used for Helvetica-BoldOblique)
// =============================================================================
#[rustfmt::skip]
static HELVETICA_BOLD: StandardFontData = StandardFontData {
    widths: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47: space ! " # $ % & ' ( ) * + , - . /
        278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
        // 48-63: 0 1 2 3 4 5 6 7 8 9 : ; < = > ?
        556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
        // 64-79: @ A B C D E F G H I J K L M N O
        975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
        // 80-95: P Q R S T U V W X Y Z [ \ ] ^ _
        667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
        // 96-111: ` a b c d e f g h i j k l m n o
        333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611,
        // 112-127: p q r s t u v w x y z { | } ~ DEL
        611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584, 0,
        // 128-143
        556, 0, 278, 556, 500, 1000, 556, 556, 333, 1000, 667, 333, 1000, 0, 611, 0,
        // 144-159
        0, 278, 278, 500, 500, 350, 556, 1000, 333, 1000, 556, 333, 944, 0, 500, 667,
        // 160-175
        278, 333, 556, 556, 556, 556, 280, 556, 333, 737, 370, 556, 584, 333, 737, 333,
        // 176-191
        400, 584, 333, 333, 333, 611, 556, 278, 333, 333, 365, 556, 834, 834, 834, 611,
        // 192-207: Agrave..Idieresis
        722, 722, 722, 722, 722, 722, 1000, 722, 667, 667, 667, 667, 278, 278, 278, 278,
        // 208-223: Eth..germandbls
        722, 722, 778, 778, 778, 778, 778, 584, 778, 722, 722, 722, 722, 667, 667, 611,
        // 224-239: agrave..idieresis
        556, 556, 556, 556, 556, 556, 889, 556, 556, 556, 556, 556, 278, 278, 278, 278,
        // 240-255: eth..ydieresis
        611, 611, 611, 611, 611, 611, 611, 584, 611, 611, 611, 611, 611, 556, 611, 556,
    ],
    font_bbox: [-170, -228, 1003, 962],
};

// =============================================================================
// Times-Roman
// =============================================================================
#[rustfmt::skip]
static TIMES_ROMAN: StandardFontData = StandardFontData {
    widths: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47
        250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278,
        // 48-63
        500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444,
        // 64-79
        921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722,
        // 80-95
        556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500,
        // 96-111
        333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500,
        // 112-127
        500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541, 0,
        // 128-143
        500, 0, 333, 500, 444, 1000, 500, 500, 333, 1000, 556, 333, 889, 0, 611, 0,
        // 144-159
        0, 333, 333, 444, 444, 350, 500, 1000, 333, 980, 389, 333, 722, 0, 444, 722,
        // 160-175
        250, 333, 500, 500, 500, 500, 200, 500, 333, 760, 276, 500, 564, 333, 760, 333,
        // 176-191
        400, 564, 300, 300, 333, 500, 453, 250, 333, 300, 310, 500, 750, 750, 750, 444,
        // 192-207
        722, 722, 722, 722, 722, 722, 889, 667, 611, 611, 611, 611, 333, 333, 333, 333,
        // 208-223
        722, 722, 722, 722, 722, 722, 722, 564, 722, 722, 722, 722, 722, 722, 556, 500,
        // 224-239
        444, 444, 444, 444, 444, 444, 667, 444, 444, 444, 444, 444, 278, 278, 278, 278,
        // 240-255
        500, 500, 500, 500, 500, 500, 500, 564, 500, 500, 500, 500, 500, 500, 500, 500,
    ],
    font_bbox: [-168, -218, 1000, 898],
};

// =============================================================================
// Times-Bold
// =============================================================================
#[rustfmt::skip]
static TIMES_BOLD: StandardFontData = StandardFontData {
    widths: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47
        250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278,
        // 48-63
        500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
        // 64-79
        930, 722, 667, 722, 722, 667, 611, 778, 778, 389, 500, 778, 667, 944, 722, 778,
        // 80-95
        611, 778, 722, 556, 667, 722, 722, 1000, 722, 722, 667, 333, 278, 333, 581, 500,
        // 96-111
        333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556, 278, 833, 556, 500,
        // 112-127
        556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520, 0,
        // 128-143
        500, 0, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 1000, 0, 667, 0,
        // 144-159
        0, 333, 333, 500, 500, 350, 500, 1000, 333, 1000, 389, 333, 722, 0, 444, 722,
        // 160-175
        250, 333, 500, 500, 500, 500, 220, 500, 333, 747, 300, 500, 570, 333, 747, 333,
        // 176-191
        400, 570, 300, 300, 333, 556, 540, 250, 333, 300, 330, 500, 750, 750, 750, 500,
        // 192-207
        722, 722, 722, 722, 722, 722, 1000, 722, 667, 667, 667, 667, 389, 389, 389, 389,
        // 208-223
        722, 722, 778, 778, 778, 778, 778, 570, 778, 722, 722, 722, 722, 722, 611, 556,
        // 224-239
        500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444, 278, 278, 278, 278,
        // 240-255
        500, 556, 500, 500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 500, 556, 500,
    ],
    font_bbox: [-168, -218, 1000, 935],
};

// =============================================================================
// Times-Italic
// =============================================================================
#[rustfmt::skip]
static TIMES_ITALIC: StandardFontData = StandardFontData {
    widths: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47
        250, 333, 420, 500, 500, 833, 778, 214, 333, 333, 500, 675, 250, 333, 250, 278,
        // 48-63
        500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 675, 675, 675, 500,
        // 64-79
        920, 611, 611, 667, 722, 611, 611, 722, 722, 333, 444, 667, 556, 833, 667, 722,
        // 80-95
        611, 722, 611, 500, 556, 722, 611, 833, 611, 556, 556, 389, 278, 389, 422, 500,
        // 96-111
        333, 500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444, 278, 722, 500, 500,
        // 112-127
        500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389, 400, 275, 400, 541, 0,
        // 128-143
        500, 0, 333, 500, 556, 889, 500, 500, 333, 1000, 500, 333, 944, 0, 556, 0,
        // 144-159
        0, 333, 333, 556, 556, 350, 500, 889, 333, 980, 389, 333, 667, 0, 389, 556,
        // 160-175
        250, 389, 500, 500, 500, 500, 275, 500, 333, 760, 276, 500, 675, 333, 760, 333,
        // 176-191
        400, 675, 300, 300, 333, 500, 523, 250, 333, 300, 310, 500, 750, 750, 750, 500,
        // 192-207
        611, 611, 611, 611, 611, 611, 889, 667, 611, 611, 611, 611, 333, 333, 333, 333,
        // 208-223
        722, 667, 722, 722, 722, 722, 722, 675, 722, 722, 722, 722, 722, 556, 611, 500,
        // 224-239
        500, 500, 500, 500, 500, 500, 667, 444, 444, 444, 444, 444, 278, 278, 278, 278,
        // 240-255
        500, 500, 500, 500, 500, 500, 500, 675, 500, 500, 500, 500, 500, 444, 500, 444,
    ],
    font_bbox: [-169, -217, 1010, 883],
};

// =============================================================================
// Times-BoldItalic
// =============================================================================
#[rustfmt::skip]
static TIMES_BOLD_ITALIC: StandardFontData = StandardFontData {
    widths: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47
        250, 389, 555, 500, 500, 833, 778, 278, 333, 333, 500, 570, 250, 333, 250, 278,
        // 48-63
        500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
        // 64-79
        832, 667, 667, 667, 722, 667, 667, 722, 778, 389, 500, 667, 611, 889, 722, 722,
        // 80-95
        611, 722, 667, 556, 611, 722, 667, 889, 667, 611, 611, 333, 278, 333, 570, 500,
        // 96-111
        333, 500, 500, 444, 500, 444, 333, 500, 556, 278, 278, 500, 278, 778, 556, 500,
        // 112-127
        500, 500, 389, 389, 278, 556, 444, 667, 500, 444, 389, 348, 220, 348, 570, 0,
        // 128-143
        500, 0, 333, 500, 500, 1000, 500, 500, 333, 1000, 556, 333, 944, 0, 611, 0,
        // 144-159
        0, 333, 333, 500, 500, 350, 500, 1000, 333, 1000, 389, 333, 667, 0, 389, 611,
        // 160-175
        250, 389, 500, 500, 500, 500, 220, 500, 333, 747, 266, 500, 606, 333, 747, 333,
        // 176-191
        400, 570, 300, 300, 333, 576, 500, 250, 333, 300, 300, 500, 750, 750, 750, 500,
        // 192-207
        667, 667, 667, 667, 667, 667, 944, 667, 667, 667, 667, 667, 389, 389, 389, 389,
        // 208-223
        722, 722, 722, 722, 722, 722, 722, 570, 722, 722, 722, 722, 722, 611, 611, 500,
        // 224-239
        500, 500, 500, 500, 500, 500, 722, 444, 444, 444, 444, 444, 278, 278, 278, 278,
        // 240-255
        500, 556, 500, 500, 500, 500, 500, 570, 500, 556, 556, 556, 556, 444, 500, 444,
    ],
    font_bbox: [-200, -218, 996, 921],
};

// =============================================================================
// Symbol (uses Symbol encoding, not WinAnsiEncoding)
// =============================================================================
#[rustfmt::skip]
static SYMBOL: StandardFontData = StandardFontData {
    widths: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47: space ! universal # existential % & suchthat ( ) * + , - . /
        250, 333, 713, 500, 549, 833, 778, 439, 333, 333, 500, 549, 250, 549, 250, 278,
        // 48-63: 0 1 2 3 4 5 6 7 8 9 : ; < = > ?
        500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 549, 549, 549, 444,
        // 64-79: congruent Alpha Beta Chi Delta Epsilon Phi Gamma Eta Iota theta1 Kappa Lambda Mu Nu Omicron
        549, 722, 667, 722, 612, 611, 763, 603, 722, 333, 631, 722, 686, 889, 722, 722,
        // 80-95: Pi Theta Rho Sigma Tau Upsilon sigma1 Omega Xi Psi Zeta [ therefore ] perpendicular _
        768, 741, 556, 592, 611, 690, 439, 768, 645, 795, 611, 333, 863, 333, 658, 500,
        // 96-111: radicalex alpha beta chi delta epsilon phi gamma eta iota phi1 kappa lambda mu nu omicron
        500, 631, 549, 549, 494, 439, 521, 411, 603, 329, 603, 549, 549, 576, 521, 549,
        // 112-127: pi theta rho sigma tau upsilon omega1 omega xi psi zeta { | } ~ DEL
        549, 521, 549, 603, 439, 576, 713, 686, 493, 686, 494, 480, 200, 480, 549, 0,
        // 128-159: mostly undefined
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 160-175
        250, 620, 247, 549, 167, 713, 500, 753, 753, 753, 753, 1042, 987, 603, 987, 603,
        // 176-191
        400, 549, 411, 549, 549, 713, 494, 460, 549, 549, 549, 549, 1000, 603, 1000, 658,
        // 192-207
        823, 686, 795, 987, 768, 768, 823, 768, 768, 713, 713, 713, 713, 713, 713, 768,
        // 208-223
        768, 713, 790, 790, 890, 823, 549, 250, 713, 603, 603, 1042, 987, 603, 987, 603,
        // 224-239
        494, 329, 790, 790, 786, 713, 384, 384, 384, 384, 384, 384, 494, 494, 494, 494,
        // 240-255
        0, 329, 274, 686, 686, 686, 384, 384, 384, 384, 384, 384, 494, 494, 494, 0,
    ],
    font_bbox: [-180, -293, 1090, 1010],
};

// =============================================================================
// ZapfDingbats (uses ZapfDingbats encoding)
// =============================================================================
#[rustfmt::skip]
static ZAPF_DINGBATS: StandardFontData = StandardFontData {
    widths: [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 32-47
        278, 974, 961, 974, 980, 719, 789, 790, 791, 690, 960, 939, 549, 855, 911, 933,
        // 48-63
        911, 945, 974, 755, 846, 762, 761, 571, 677, 763, 760, 759, 754, 494, 552, 537,
        // 64-79
        577, 692, 786, 788, 788, 790, 793, 794, 816, 823, 789, 841, 823, 833, 816, 831,
        // 80-95
        923, 744, 723, 749, 790, 792, 695, 776, 768, 792, 759, 707, 708, 682, 701, 826,
        // 96-111
        815, 789, 789, 707, 687, 696, 689, 786, 787, 713, 791, 785, 791, 873, 761, 762,
        // 112-127
        762, 759, 759, 892, 892, 788, 784, 438, 138, 277, 415, 392, 392, 668, 668, 0,
        // 128-159: undefined
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // 160-175
        0, 732, 544, 544, 910, 667, 760, 760, 776, 595, 694, 626, 788, 788, 788, 788,
        // 176-191
        788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788,
        // 192-207
        788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788, 788,
        // 208-223
        788, 788, 788, 788, 894, 838, 1016, 458, 748, 924, 748, 918, 927, 928, 928, 834,
        // 224-239
        873, 828, 924, 924, 917, 930, 931, 463, 883, 836, 836, 867, 867, 696, 696, 874,
        // 240-255
        0, 874, 760, 946, 771, 865, 771, 888, 967, 888, 831, 873, 927, 970, 918, 0,
    ],
    font_bbox: [-1, -143, 981, 820],
};

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Known width values (from acceptance criteria) ==========

    #[test]
    fn helvetica_known_widths() {
        let data = lookup("Helvetica").unwrap();
        assert_eq!(data.widths[65], 667, "Helvetica 'A' should be 667");
        assert_eq!(data.widths[32], 278, "Helvetica space should be 278");
    }

    #[test]
    fn courier_known_widths() {
        let data = lookup("Courier").unwrap();
        assert_eq!(data.widths[65], 600, "Courier 'A' should be 600");
        assert_eq!(data.widths[32], 600, "Courier space should be 600");
    }

    #[test]
    fn courier_all_uniform_600() {
        for name in &[
            "Courier",
            "Courier-Bold",
            "Courier-Oblique",
            "Courier-BoldOblique",
        ] {
            let data = lookup(name).unwrap();
            for (i, &w) in data.widths.iter().enumerate() {
                assert_eq!(w, 600, "{name} width at code {i} should be 600");
            }
        }
    }

    // ========== All 14 fonts present ==========

    #[test]
    fn all_14_standard_fonts_present() {
        let names = [
            "Courier",
            "Courier-Bold",
            "Courier-Oblique",
            "Courier-BoldOblique",
            "Helvetica",
            "Helvetica-Bold",
            "Helvetica-Oblique",
            "Helvetica-BoldOblique",
            "Times-Roman",
            "Times-Bold",
            "Times-Italic",
            "Times-BoldItalic",
            "Symbol",
            "ZapfDingbats",
        ];
        for name in &names {
            assert!(lookup(name).is_some(), "lookup({name}) should return Some");
        }
    }

    // ========== Unknown font returns None ==========

    #[test]
    fn unknown_font_returns_none() {
        assert!(lookup("Arial").is_none());
        assert!(lookup("UnknownFont").is_none());
        assert!(lookup("").is_none());
        assert!(lookup("helvetica").is_none()); // case sensitive
    }

    // ========== Oblique variants share widths with upright ==========

    #[test]
    fn helvetica_oblique_same_as_regular() {
        let regular = lookup("Helvetica").unwrap();
        let oblique = lookup("Helvetica-Oblique").unwrap();
        assert_eq!(regular.widths, oblique.widths);
    }

    #[test]
    fn helvetica_bold_oblique_same_as_bold() {
        let bold = lookup("Helvetica-Bold").unwrap();
        let bold_oblique = lookup("Helvetica-BoldOblique").unwrap();
        assert_eq!(bold.widths, bold_oblique.widths);
    }

    // ========== Font bbox values ==========

    #[test]
    fn font_bbox_values() {
        let courier = lookup("Courier").unwrap();
        assert_eq!(courier.font_bbox, [-23, -250, 715, 805]);

        let helvetica = lookup("Helvetica").unwrap();
        assert_eq!(helvetica.font_bbox, [-166, -225, 1000, 931]);

        let times = lookup("Times-Roman").unwrap();
        assert_eq!(times.font_bbox, [-168, -218, 1000, 898]);
    }

    // ========== Proportional font widths vary ==========

    #[test]
    fn helvetica_widths_are_proportional() {
        let data = lookup("Helvetica").unwrap();
        // 'i' is narrow, 'M' is wide
        assert!(data.widths[105] < data.widths[77], "i < M");
        // 'W' is wider than 'I'
        assert!(data.widths[87] > data.widths[73], "W > I");
    }

    #[test]
    fn times_roman_widths_are_proportional() {
        let data = lookup("Times-Roman").unwrap();
        assert!(data.widths[105] < data.widths[77], "i < M");
    }

    // ========== StandardFontData struct fields ==========

    #[test]
    fn standard_font_data_has_widths_and_bbox() {
        let data = lookup("Helvetica").unwrap();
        assert_eq!(data.widths.len(), 256);
        assert_eq!(data.font_bbox.len(), 4);
    }
}
