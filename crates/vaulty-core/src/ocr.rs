//! Text recognised in a photo (by the operating system's own text recognition, or Tesseract) as
//! lines in reading order, laid out the way `pdf_text` lays out a PDF's text: pieces far apart on
//! a line are joined with two spaces, so a bill's columns stay apart for `invoice::parse`.
//!
//! Recognition itself happens in the app shell (it's different on every system); this module only
//! arranges what it found, so it can be tested anywhere.
use serde::{Deserialize, Serialize};

/// A recognised piece of text (a word or a run of words) and where it is: left, top, width and
/// height as fractions of the image (top-left origin), so every recogniser gives the same thing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Piece { pub text: String, pub x: f64, pub y: f64, pub w: f64, pub h: f64 }

/// Lines, top to bottom. Pieces whose middles are within half a line height of each other are on
/// one line; a gap wider than about two characters separates columns.
pub fn lines(pieces: &[Piece]) -> Vec<String> {
    let mut ps: Vec<&Piece> = pieces.iter().filter(|p| !p.text.trim().is_empty() && p.h > 0.0 && p.w >= 0.0).collect();
    ps.sort_by(|a, b| (a.y + a.h / 2.0).total_cmp(&(b.y + b.h / 2.0)));
    let mut rows: Vec<Vec<&Piece>> = vec![];
    for p in ps {
        let mid = p.y + p.h / 2.0;
        match rows.last_mut() {
            Some(row) => {
                let (rmid, rh) = row.iter().fold((0.0, 0.0), |(m, h), q| (m + q.y + q.h / 2.0, h + q.h));
                let n = row.len() as f64;
                if (mid - rmid / n).abs() <= (rh / n).max(p.h) * 0.5 { row.push(p) } else { rows.push(vec![p]) }
            }
            None => rows.push(vec![p]),
        }
    }
    rows.into_iter().map(|mut row| {
        row.sort_by(|a, b| a.x.total_cmp(&b.x));
        let mut line = String::new();
        let mut end = f64::MIN;
        for p in row {
            let text = p.text.trim();
            if !line.is_empty() {
                // About one character's width: the piece's width over its length.
                let ch = (p.w / text.chars().count().max(1) as f64).max(p.h * 0.3);
                line.push_str(if p.x - end > ch * 2.2 { "  " } else { " " });
            }
            line.push_str(text);
            end = p.x + p.w;
        }
        line
    }).filter(|l| !l.is_empty()).collect()
}

/// Pieces from Tesseract's TSV output (`tesseract image stdout tsv`): its words with their
/// boxes in pixels, scaled to fractions of the page.
pub fn from_tesseract_tsv(tsv: &str) -> Vec<Piece> {
    let mut page = (1.0f64, 1.0f64);
    let mut out = vec![];
    for row in tsv.lines().skip(1) {
        let c: Vec<&str> = row.split('\t').collect();
        if c.len() < 12 { continue; }
        let n = |i: usize| c[i].trim().parse::<f64>().unwrap_or(0.0);
        match c[0].trim() {
            "1" => page = (n(8).max(1.0), n(9).max(1.0)),
            "5" if n(10) >= 0.0 && !c[11].trim().is_empty() => {
                out.push(Piece { text: c[11].trim().to_string(), x: n(6) / page.0, y: n(7) / page.1, w: n(8) / page.0, h: n(9) / page.1 });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(text: &str, x: f64, y: f64, w: f64) -> Piece { Piece { text: text.into(), x, y, w, h: 0.02 } }

    #[test]
    fn columns_and_rows() {
        let pieces = vec![
            p("7,402.00", 0.85, 0.301, 0.1), p("Copper", 0.1, 0.3, 0.08), p("wire", 0.19, 0.302, 0.05), p("4", 0.6, 0.299, 0.01),
            p("TAX INVOICE", 0.4, 0.1, 0.2),
            p("1,850.50", 0.72, 0.3, 0.09),
        ];
        assert_eq!(lines(&pieces), ["TAX INVOICE", "Copper wire  4  1,850.50  7,402.00"]);
        assert!(lines(&[]).is_empty());
    }

    #[test]
    fn tesseract_tsv() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t1000\t2000\t-1\t\n\
                   4\t1\t1\t1\t1\t0\t100\t200\t500\t40\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t100\t200\t120\t40\t96.5\tPrimer\n\
                   5\t1\t1\t1\t1\t2\t700\t200\t150\t40\t91.0\t2300.00\n";
        let ps = from_tesseract_tsv(tsv);
        assert_eq!(ps.len(), 2);
        assert_eq!((ps[0].x, ps[0].y, ps[1].w), (0.1, 0.1, 0.15));
        assert_eq!(lines(&ps), ["Primer  2300.00"]);
    }
}
