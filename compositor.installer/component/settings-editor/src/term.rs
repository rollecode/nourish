//! A tiny terminal layer for single-keypress navigation — just enough to read a
//! real Escape key, arrows, digits and Enter, with NO TUI dependency. On a TTY it
//! flips the terminal into raw mode (via `rustix::termios`) for the duration of one
//! key read, then restores the prior settings. When stdin is NOT a TTY (piped runs,
//! CI, `--write-default`), `read_key` returns [`Key::Eof`] immediately so callers
//! fall back to their non-interactive default instead of blocking.

use rustix::termios::{self, Termios};
use std::io::Read;
use std::os::fd::AsFd;

/// A single decoded keypress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Esc,
    Up,
    Down,
    Char(char),
    /// Non-TTY stdin, EOF, or a read error — callers should keep their default.
    Eof,
    /// Anything we don't act on (kept distinct from `Eof` so loops don't exit).
    Other,
}

/// The outcome of a selection screen: a chosen value, or a request to go back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav<T> {
    Selected(T),
    Back,
}

/// Is stdin an interactive terminal? Selection helpers only enter raw mode when so.
pub fn is_tty() -> bool {
    termios::isatty(std::io::stdin())
}

/// RAII guard: put the terminal into raw mode on construction, restore on drop.
struct RawGuard {
    saved: Option<Termios>,
}

impl RawGuard {
    fn new() -> Self {
        let stdin = std::io::stdin();
        let saved = termios::tcgetattr(stdin.as_fd()).ok();
        if let Some(prev) = &saved {
            let mut raw = prev.clone();
            raw.make_raw();
            // Best-effort: if we can't enter raw mode, reads still work line-buffered.
            let _ = termios::tcsetattr(stdin.as_fd(), termios::OptionalActions::Now, &raw);
        }
        Self { saved }
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.saved {
            let _ = termios::tcsetattr(std::io::stdin().as_fd(), termios::OptionalActions::Now, prev);
        }
    }
}

/// Read and decode one keypress in raw mode. Returns [`Key::Eof`] on non-TTY stdin
/// so non-interactive runs never block. Recognizes Enter, Escape (a bare ESC with
/// nothing following), the Up/Down arrow CSI sequences, digits and printable chars.
pub fn read_key() -> Key {
    if !is_tty() {
        return Key::Eof;
    }
    let _guard = RawGuard::new();
    let mut stdin = std::io::stdin();

    let mut b = [0u8; 1];
    match stdin.read(&mut b) {
        Ok(0) | Err(_) => return Key::Eof,
        Ok(_) => {}
    }
    match b[0] {
        b'\r' | b'\n' => Key::Enter,
        0x03 => Key::Eof, // Ctrl-C: treat as give-up so we never trap the user.
        0x1B => {
            // ESC alone is "back"; ESC '[' starts a CSI arrow sequence. Peek one
            // more byte non-blockingly via a short VMIN/VTIME read.
            match peek_byte() {
                Some(b'[') => match peek_byte() {
                    Some(b'A') => Key::Up,
                    Some(b'B') => Key::Down,
                    _ => Key::Other,
                },
                _ => Key::Esc,
            }
        }
        c if c.is_ascii_graphic() || c == b' ' => Key::Char(c as char),
        _ => Key::Other,
    }
}

/// Read one more byte with a tiny timeout so a lone ESC doesn't hang waiting for a
/// CSI continuation. Uses VMIN=0/VTIME=1 (0.1s) on the already-raw terminal.
fn peek_byte() -> Option<u8> {
    let stdin = std::io::stdin();
    let prev = termios::tcgetattr(stdin.as_fd()).ok()?;
    let mut t = prev.clone();
    t.special_codes[termios::SpecialCodeIndex::VMIN] = 0;
    t.special_codes[termios::SpecialCodeIndex::VTIME] = 1;
    let _ = termios::tcsetattr(stdin.as_fd(), termios::OptionalActions::Now, &t);
    let mut b = [0u8; 1];
    let got = std::io::stdin().read(&mut b).ok().filter(|n| *n == 1).map(|_| b[0]);
    let _ = termios::tcsetattr(stdin.as_fd(), termios::OptionalActions::Now, &prev);
    got
}
