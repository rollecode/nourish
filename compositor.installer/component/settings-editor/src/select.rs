//! A numbered list selector built on [`crate::term`]. Renders an item list (the
//! marked item, if any, shown with `*`), then reads keys: digits jump to an item,
//! Up/Down move a highlight, Enter confirms the highlight, and Escape goes back (when
//! `allow_back`). On non-TTY stdin it degrades to a single line read (number to pick,
//! blank to keep the marked item, `b` to go back) so piped/headless runs never block.

use crate::term::{self, Key, Nav};
use std::io::Write;

/// One selectable row: a primary `label` and an optional dim `detail` (e.g. the
/// device node or the saved-mode hint) shown after it.
pub struct Item {
    pub label: String,
    pub detail: String,
}

impl Item {
    pub fn new(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { label: label.into(), detail: detail.into() }
    }
}

/// Present `items` under `title`. `marked` is the index to flag with `*` (the saved
/// value / current default), or `None` when there is no meaningful current — then no
/// item is starred and the highlight simply starts at the top. Returns the chosen
/// index, or [`Nav::Back`] when the user escapes (only possible when `allow_back`).
/// An empty `items` slice always returns `Back`.
pub fn select_list(title: &str, items: &[Item], marked: Option<usize>, allow_back: bool) -> Nav<usize> {
    if items.is_empty() {
        return Nav::Back;
    }
    if !term::is_tty() {
        return line_select(title, items, marked, allow_back);
    }
    let mut hi = marked.unwrap_or(0).min(items.len() - 1);
    loop {
        render(title, items, hi, marked, allow_back);
        match term::read_key() {
            Key::Up => hi = if hi == 0 { items.len() - 1 } else { hi - 1 },
            Key::Down => hi = (hi + 1) % items.len(),
            Key::Enter => return Nav::Selected(hi),
            Key::Esc if allow_back => return Nav::Back,
            Key::Eof => return Nav::Back,
            Key::Char(c) if c.is_ascii_digit() => {
                let n = (c as u8 - b'0') as usize;
                if n >= 1 && n <= items.len() {
                    return Nav::Selected(n - 1);
                }
            }
            _ => {}
        }
    }
}

/// Draw the list, re-printing in place each keypress (simple scroll, no cursor math).
fn render(title: &str, items: &[Item], hi: usize, marked: Option<usize>, allow_back: bool) {
    println!("\n{title}");
    for (i, it) in items.iter().enumerate() {
        let arrow = if i == hi { ">" } else { " " };
        let star = if marked == Some(i) { "*" } else { " " };
        let detail = if it.detail.is_empty() { String::new() } else { format!("  ({})", it.detail) };
        println!("  {arrow} {star} {}) {}{}", i + 1, it.label, detail);
    }
    let hint = if allow_back {
        "↑/↓ move · Enter select · number to jump · Esc back"
    } else {
        "↑/↓ move · Enter select · number to jump"
    };
    print!("  {hint} ");
    let _ = std::io::stdout().flush();
}

/// Non-TTY fallback: print the list once and read a single line. With a `marked`
/// item, a blank line keeps it; with none, a blank line (or EOF) goes Back.
fn line_select(title: &str, items: &[Item], marked: Option<usize>, allow_back: bool) -> Nav<usize> {
    println!("\n{title}");
    for (i, it) in items.iter().enumerate() {
        let star = if marked == Some(i) { " *" } else { "" };
        let detail = if it.detail.is_empty() { String::new() } else { format!("  ({})", it.detail) };
        println!("    {}) {}{}{}", i + 1, it.label, detail, star);
    }
    let back = if allow_back { ", 'b' to go back" } else { "" };
    match marked {
        Some(d) => print!("  choice [{}]{back} ", d + 1),
        None => print!("  choice{back} "),
    }
    let _ = std::io::stdout().flush();

    let mut s = String::new();
    if std::io::stdin().read_line(&mut s).unwrap_or(0) == 0 {
        // EOF: give up. The menu loop treats this as exit; a field selector's caller
        // maps Back to "keep current", so either way we don't loop on a closed stdin.
        return Nav::Back;
    }
    let s = s.trim();
    if s.is_empty() {
        // Blank keeps the marked item, or backs out when nothing is marked.
        return marked.map_or(Nav::Back, Nav::Selected);
    }
    if allow_back && s.eq_ignore_ascii_case("b") {
        return Nav::Back;
    }
    match s.parse::<usize>() {
        Ok(n) if n >= 1 && n <= items.len() => Nav::Selected(n - 1),
        _ => marked.map_or(Nav::Back, Nav::Selected),
    }
}
