//! Tokenize raw PDF content stream bytes into typed [`Operator`] + [`Operand`] pairs.
//!
//! The tokenizer is the lowest layer of pdfplumber-parse. Every content-stream
//! operator (`BT`, `Tj`, `cm`, `re`, …) and its operand list passes through here
//! before the interpreter processes it.
//!
//! Run with: `cargo run --example tokenize_stream -p pdfplumber-parse`

use pdfplumber_parse::{Operand, tokenize, tokenize_lenient};

fn print_ops(label: &str, stream: &[u8]) {
    println!("── {label} ──────────────────────────────────────────────────");
    println!(
        "   input: {}",
        std::str::from_utf8(stream).unwrap_or("<binary>")
    );

    match tokenize(stream) {
        Ok(ops) => {
            for op in &ops {
                let operands = op
                    .operands
                    .iter()
                    .map(|o| match o {
                        Operand::Integer(i) => format!("{i}"),
                        Operand::Real(r) => format!("{r:.3}"),
                        Operand::Name(n) => format!("/{n}"),
                        Operand::LiteralString(b) => format!("({:?})", String::from_utf8_lossy(b)),
                        Operand::HexString(b) => format!("<{}>", hex(b)),
                        Operand::Boolean(v) => format!("{v}"),
                        Operand::Null => "null".into(),
                        Operand::Array(a) => format!("[{} items]", a.len()),
                        Operand::Dictionary(d) => format!("<<{} keys>>", d.len()),
                    })
                    .collect::<Vec<_>>()
                    .join("  ");
                if operands.is_empty() {
                    println!("   {:6}", op.operator);
                } else {
                    println!("   {:6}  ←  {operands}", op.operator);
                }
            }
        }
        Err(e) => println!("   error: {e}"),
    }
    println!();
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    // Text object: BT … ET with font selection and text positioning
    print_ops("text object", b"BT /F1 12 Tf 72 720 Td (Hello, PDF!) Tj ET");

    // Graphics state: save/restore, CTM, fill colour, rectangle, fill
    print_ops(
        "graphics state",
        b"q 1 0 0 1 100 200 cm 0.5 g 0 0 200 100 re f Q",
    );

    // TJ operator: array of strings and glyph-spacing adjustments
    print_ops("TJ operator", b"[(Kern)-120(ing) 80( demo)] TJ");

    // Lenient mode: skips unknown tokens instead of returning Err
    let broken: &[u8] = b"BT /F1 12 Tf (valid text) Tj @@INVALID@@ ET";
    println!("── lenient mode (malformed input) ──────────────────────────────");
    println!("   input: {}", String::from_utf8_lossy(broken));
    let recovered = tokenize_lenient(broken);
    println!(
        "   recovered {}/{} operator(s) past garbage token",
        recovered.len(),
        tokenize(broken)
            .map(|v| v.len())
            .unwrap_or(0)
            .max(recovered.len())
    );
    for op in &recovered {
        println!("   {:6}  ({} operand(s))", op.operator, op.operands.len());
    }
}
