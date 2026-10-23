//! Text from a PDF as lines in reading order, rebuilt from where each piece of text sits on the
//! page (statements lay out their columns by position, so the order in the file isn't the order
//! on the page). Pieces far apart on a line are joined with two spaces, near ones with one.
//!
//! Password-protected PDFs (RC4 and AES, as banks and fund registrars use) are opened with the
//! password. Files and pages are size-limited, since they come from outside.
use crate::error::{Error, Result};
use lopdf::{content::Content, Document, Encoding, Object};
use std::collections::BTreeMap;

/// The largest PDF read, and the most any one page may decompress to.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;
const MAX_PAGE_CONTENT: usize = 8 * 1024 * 1024;
pub const MAX_PAGES: usize = 400;

/// Opens a PDF, with its password if it has one.
pub fn open(bytes: &[u8], password: Option<&str>) -> Result<Document> {
    if bytes.len() > MAX_BYTES { return Err(Error::Invalid("that file is larger than 20 MB".into())); }
    if !bytes.starts_with(b"%PDF") { return Err(Error::Invalid("that isn't a PDF".into())); }
    let pw = password.map(str::trim).unwrap_or_default();
    let mut options = lopdf::LoadOptions::with_password(pw);
    // Compressed streams can't balloon while loading.
    options.max_decompressed_size = Some(MAX_PAGE_CONTENT);
    let doc = Document::load_mem_with_options(bytes, options).map_err(|_| {
        if pw.is_empty() { Error::Invalid("this PDF needs its password".into()) } else { Error::Auth("that password doesn't open this PDF".into()) }
    })?;
    if doc.is_encrypted() && doc.encryption_state.is_none() {
        return Err(if pw.is_empty() { Error::Invalid("this PDF needs its password".into()) } else { Error::Auth("that password doesn't open this PDF".into()) });
    }
    if doc.get_pages().len() > MAX_PAGES { return Err(Error::Invalid("that PDF has too many pages".into())); }
    Ok(doc)
}

#[derive(Clone, Copy)]
struct M([f64; 6]);
impl M {
    const I: M = M([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    fn mul(self, o: M) -> M {
        let (a, b) = (self.0, o.0);
        M([a[0] * b[0] + a[1] * b[2], a[0] * b[1] + a[1] * b[3], a[2] * b[0] + a[3] * b[2], a[2] * b[1] + a[3] * b[3],
           a[4] * b[0] + a[5] * b[2] + b[4], a[4] * b[1] + a[5] * b[3] + b[5]])
    }
    fn translate(x: f64, y: f64) -> M { M([1.0, 0.0, 0.0, 1.0, x, y]) }
}

fn num(o: &Object) -> f64 {
    match o { Object::Integer(i) => *i as f64, Object::Real(r) => *r as f64, _ => 0.0 }
}

struct Piece { x: f64, y: f64, size: f64, text: String }

fn page_pieces(doc: &Document, page_id: lopdf::ObjectId) -> Result<Vec<Piece>> {
    let encodings: BTreeMap<Vec<u8>, Encoding> = doc.get_page_fonts(page_id).unwrap_or_default().into_iter()
        .filter_map(|(name, font)| font.get_font_encoding_with_limit(doc, MAX_PAGE_CONTENT).ok().map(|e| (name, e)))
        .collect();
    let data = doc.get_page_content_with_limit(page_id, MAX_PAGE_CONTENT).map_err(|_| Error::Invalid("a page of that PDF is too large".into()))?;
    let content = Content::decode(&data).map_err(|_| Error::Invalid("a page of that PDF couldn't be read".into()))?;
    let (mut ctm, mut stack) = (M::I, vec![]);
    let (mut tm, mut tlm, mut leading) = (M::I, M::I, 0.0f64);
    let (mut font, mut size) = (Vec::new(), 10.0f64);
    let mut out = vec![];
    let decode = |font: &[u8], bytes: &[u8]| -> String {
        match encodings.get(font) {
            Some(e) => Document::decode_text(e, bytes).unwrap_or_default(),
            None => bytes.iter().map(|&b| b as char).collect(),
        }
    };
    for op in &content.operations {
        let o = &op.operands;
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap_or(M::I),
            "cm" if o.len() == 6 => ctm = M([num(&o[0]), num(&o[1]), num(&o[2]), num(&o[3]), num(&o[4]), num(&o[5])]).mul(ctm),
            "BT" => { tm = M::I; tlm = M::I; }
            "Tf" if o.len() == 2 => { font = o[0].as_name().map(<[u8]>::to_vec).unwrap_or_default(); size = num(&o[1]); }
            "TL" if !o.is_empty() => leading = num(&o[0]),
            "Tm" if o.len() == 6 => { tm = M([num(&o[0]), num(&o[1]), num(&o[2]), num(&o[3]), num(&o[4]), num(&o[5])]); tlm = tm; }
            "Td" | "TD" if o.len() == 2 => {
                if op.operator == "TD" { leading = -num(&o[1]); }
                tlm = M::translate(num(&o[0]), num(&o[1])).mul(tlm); tm = tlm;
            }
            "T*" => { tlm = M::translate(0.0, -leading).mul(tlm); tm = tlm; }
            "Tj" | "'" | "\"" | "TJ" => {
                if op.operator != "Tj" && op.operator != "TJ" { tlm = M::translate(0.0, -leading).mul(tlm); tm = tlm; }
                let mut text = String::new();
                let parts: Vec<&Object> = match (op.operator.as_str(), o.last()) {
                    ("TJ", Some(Object::Array(a))) => a.iter().collect(),
                    (_, Some(last)) => vec![last],
                    _ => vec![],
                };
                for p in parts {
                    match p {
                        Object::String(bytes, _) => text.push_str(&decode(&font, bytes)),
                        // A large backwards kern is a gap between words.
                        n @ (Object::Integer(_) | Object::Real(_)) if num(n) < -250.0 => text.push(' '),
                        _ => {}
                    }
                }
                let at = tm.mul(ctm);
                let scale = (at.0[2].powi(2) + at.0[3].powi(2)).sqrt().max(0.01);
                let eff = size * scale;
                if !text.trim().is_empty() { out.push(Piece { x: at.0[4], y: at.0[5], size: eff, text: text.clone() }); }
                // Move along by about the text's width (no font metrics: half the size per character).
                let advance = text.chars().count() as f64 * size * 0.5;
                tm = M::translate(advance, 0.0).mul(tm);
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Every page's lines, top to bottom.
pub fn lines(doc: &Document) -> Result<Vec<Vec<String>>> {
    let mut pages = vec![];
    for (_, id) in doc.get_pages() {
        let mut pieces = page_pieces(doc, id)?;
        pieces.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
        let mut rows: Vec<Vec<Piece>> = vec![];
        for p in pieces {
            match rows.last_mut() {
                Some(row) if (row[0].y - p.y).abs() <= row[0].size.max(p.size) * 0.4 => row.push(p),
                _ => rows.push(vec![p]),
            }
        }
        let mut out = vec![];
        for mut row in rows {
            row.sort_by(|a, b| a.x.total_cmp(&b.x));
            let mut line = String::new();
            let mut end = f64::MIN;
            for p in &row {
                if !line.is_empty() {
                    let gap = p.x - end;
                    line.push_str(if gap > p.size * 1.2 { "  " } else if gap > p.size * 0.1 || !line.ends_with(' ') { " " } else { "" });
                }
                line.push_str(p.text.trim());
                end = p.x + p.text.trim().chars().count() as f64 * p.size * 0.5;
            }
            let line = line.trim().to_string();
            if !line.is_empty() { out.push(line); }
        }
        pages.push(out);
    }
    Ok(pages)
}
