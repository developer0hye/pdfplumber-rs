//! `pdfplumber signatures` — list and verify PDF digital signatures.
//!
//! Without the `signatures` feature (or with `--no-verify`), prints only the
//! metadata fields. With `signatures` feature enabled, also performs full
//! cryptographic verification.

use std::path::Path;

pub fn run(
    file: &Path,
    verify: bool,
    format: &crate::cli::SignaturesFormat,
    password: Option<&str>,
) -> Result<(), i32> {
    let pdf = crate::shared::open_pdf_full(file, None, password).map_err(|e| {
        eprintln!("error: {e}");
        1i32
    })?;

    let infos = pdf.signatures().map_err(|e| {
        eprintln!("error reading signatures: {e}");
        1i32
    })?;

    if infos.is_empty() {
        match format {
            crate::cli::SignaturesFormat::Json => println!("[]"),
            crate::cli::SignaturesFormat::Text => println!("(no signature fields found)"),
        }
        return Ok(());
    }

    // Optionally load raw bytes for verification
    #[cfg(feature = "signatures")]
    let (raw_sigs, file_bytes) = if verify {
        let raw = pdf.raw_signatures().map_err(|e| {
            eprintln!("error reading raw signatures: {e}");
            1i32
        })?;
        let bytes = std::fs::read(file).map_err(|e| {
            eprintln!("error reading file for verification: {e}");
            1i32
        })?;
        (raw, bytes)
    } else {
        (vec![], vec![])
    };

    match format {
        crate::cli::SignaturesFormat::Text => {
            for (i, info) in infos.iter().enumerate() {
                println!("signature {}", i + 1);
                println!("  signed:       {}", info.is_signed);
                if let Some(name) = &info.signer_name {
                    println!("  signer name:  {name}");
                }
                if let Some(date) = &info.sign_date {
                    println!("  date:         {date}");
                }
                if let Some(reason) = &info.reason {
                    println!("  reason:       {reason}");
                }
                if let Some(loc) = &info.location {
                    println!("  location:     {loc}");
                }
                if let Some(filter) = &info.filter {
                    println!("  filter:       {filter}");
                }
                if let Some(sf) = &info.sub_filter {
                    println!("  sub-filter:   {sf}");
                }
                if let Some(br) = &info.byte_range {
                    println!(
                        "  byte-range:   [{}, {}, {}, {}]",
                        br[0], br[1], br[2], br[3]
                    );
                }

                #[cfg(feature = "signatures")]
                if verify {
                    if let Some(raw) = raw_sigs.get(i) {
                        let v = pdfplumber::signatures::verify_signature(raw, &file_bytes);
                        println!("  crypto-valid: {}", v.is_valid);
                        if let Some(cn) = &v.signer_cn {
                            println!("  signer CN:    {cn}");
                        }
                        if let Some(email) = &v.signer_email {
                            println!("  signer email: {email}");
                        }
                        println!("  covers doc:   {}", v.covers_entire_document);
                        if let Some(alg) = &v.digest_algorithm {
                            println!("  digest alg:   {alg}");
                        }
                        if !v.cert_chain.is_empty() {
                            println!("  cert chain ({} certs):", v.cert_chain.len());
                            for (ci, cert) in v.cert_chain.iter().enumerate() {
                                let cn = cert.subject_cn.as_deref().unwrap_or("?");
                                let issuer = cert.issuer_cn.as_deref().unwrap_or("?");
                                let self_signed = if cert.is_self_signed {
                                    " (self-signed)"
                                } else {
                                    ""
                                };
                                println!("    [{ci}] {cn} ← {issuer}{self_signed}");
                                if let Some(fp) = &cert.sha256_fingerprint {
                                    println!("        SHA-256: {}", &fp[..fp.len().min(47)]);
                                }
                            }
                        }
                        if let Some(err) = &v.error {
                            println!("  error:        {err}");
                        }
                    }
                }

                #[cfg(not(feature = "signatures"))]
                if verify {
                    println!(
                        "  crypto-valid: (not available — compile with --features signatures)"
                    );
                }

                if i + 1 < infos.len() {
                    println!();
                }
            }
        }
        crate::cli::SignaturesFormat::Json => {
            #[cfg(feature = "signatures")]
            let verifications: Vec<Option<pdfplumber::SignatureVerification>> = if verify {
                raw_sigs
                    .iter()
                    .map(|raw| Some(pdfplumber::signatures::verify_signature(raw, &file_bytes)))
                    .collect()
            } else {
                infos.iter().map(|_| None).collect()
            };

            #[cfg(not(feature = "signatures"))]
            let _verifications: Vec<Option<()>> = infos.iter().map(|_| None).collect();

            #[allow(clippy::unused_enumerate_index)]
            let json_array: Vec<serde_json::Value> = infos
                .iter()
                .enumerate()
                .map(|(_i, info)| {
                    #[allow(unused_mut)]
                    let mut obj = serde_json::json!({
                        "is_signed": info.is_signed,
                        "signer_name": info.signer_name,
                        "sign_date": info.sign_date,
                        "reason": info.reason,
                        "location": info.location,
                        "contact_info": info.contact_info,
                        "filter": info.filter,
                        "sub_filter": info.sub_filter,
                        "byte_range": info.byte_range.map(|br| serde_json::json!([br[0], br[1], br[2], br[3]])),
                    });

                    #[cfg(feature = "signatures")]
                    if let Some(Some(v)) = verifications.get(_i) {
                        obj["verification"] = serde_json::json!({
                            "is_valid": v.is_valid,
                            "signer_cn": v.signer_cn,
                            "signer_email": v.signer_email,
                            "covers_entire_document": v.covers_entire_document,
                            "digest_algorithm": v.digest_algorithm,
                            "error": v.error,
                            "cert_chain": v.cert_chain.iter().map(|c| serde_json::json!({
                                "subject_cn": c.subject_cn,
                                "issuer_cn": c.issuer_cn,
                                "not_before": c.not_before,
                                "not_after": c.not_after,
                                "sha256_fingerprint": c.sha256_fingerprint,
                                "is_self_signed": c.is_self_signed,
                            })).collect::<Vec<_>>(),
                        });
                    }

                    obj
                })
                .collect();

            println!("{}", serde_json::to_string_pretty(&json_array).unwrap());
        }
    }

    Ok(())
}
