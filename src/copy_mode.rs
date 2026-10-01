use crossterm::event::{KeyCode, KeyModifiers};

use crate::input::TerminalKey;

pub(crate) fn first_non_blank_col(text: &str) -> Option<u16> {
    let mut col = 0u16;
    for ch in text.chars() {
        if !ch.is_whitespace() {
            return Some(col);
        }
        col = col.saturating_add(char_cell_width(ch));
    }
    None
}

pub(crate) fn last_character_col(text: &str) -> Option<u16> {
    let mut col = 0u16;
    let mut last_col = None;
    for ch in text.chars() {
        let width = u16::from(crate::ghostty::unicode_codepoint_width(ch as u32));
        if width > 0 {
            last_col = Some(col);
            col = col.saturating_add(width);
        }
    }
    last_col
}

fn char_cell_width(ch: char) -> u16 {
    u16::from(crate::ghostty::unicode_codepoint_width(ch as u32)).max(1)
}

pub(crate) fn copy_mode_page_lines(height: u16, half_page: bool) -> usize {
    if height <= 2 {
        1
    } else if half_page {
        usize::from(height / 2)
    } else {
        usize::from(height - 2)
    }
}

pub(crate) fn copy_mode_command_char(key: TerminalKey) -> Option<char> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    if let Some(ch) = key.shifted_codepoint.and_then(char::from_u32) {
        return Some(ch);
    }
    let KeyCode::Char(ch) = key.code else {
        return None;
    };
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        Some(shifted_ascii_char(ch).unwrap_or(ch))
    } else {
        Some(ch)
    }
}

fn shifted_ascii_char(ch: char) -> Option<char> {
    match ch {
        'a'..='z' => Some(ch.to_ascii_uppercase()),
        '1' => Some('!'),
        '2' => Some('@'),
        '3' => Some('#'),
        '4' => Some('$'),
        '5' => Some('%'),
        '6' => Some('^'),
        '7' => Some('&'),
        '8' => Some('*'),
        '9' => Some('('),
        '0' => Some(')'),
        '-' => Some('_'),
        '=' => Some('+'),
        '[' => Some('{'),
        ']' => Some('}'),
        '\\' => Some('|'),
        ';' => Some(':'),
        '\'' => Some('"'),
        ',' => Some('<'),
        '.' => Some('>'),
        '/' => Some('?'),
        '`' => Some('~'),
        _ => None,
    }
}

/// Commands in the emacs copy-mode key table, modelled on tmux's `mode-keys emacs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmacsCopyCommand {
    Cancel,
    ClearSelection,
    BeginSelection,
    CopyAndExit,
    CursorLeft,
    CursorRight,
    CursorUp,
    CursorDown,
    LineStart,
    LineEnd,
    FirstNonBlank,
    NextWordEnd,
    PreviousWordStart,
    PreviousParagraph,
    NextParagraph,
    PageUp,
    PageDown,
    HistoryTop,
    HistoryBottom,
    SearchForward,
    SearchBackward,
    SearchAgain,
    SearchReverse,
}

/// Resolves a key to the character it produces and whether Ctrl/Alt were held.
fn emacs_key_parts(key: &TerminalKey) -> Option<(char, bool, bool)> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    if key
        .modifiers
        .difference(KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT)
        .bits()
        != 0
    {
        return None;
    }
    let KeyCode::Char(code) = key.code else {
        return None;
    };
    let ch = match key.shifted_codepoint.and_then(char::from_u32) {
        Some(shifted) => shifted,
        None if key.modifiers.contains(KeyModifiers::SHIFT) => {
            shifted_ascii_char(code).unwrap_or(code)
        }
        None => code,
    };
    Some((ch, ctrl, alt))
}

pub(crate) fn copy_mode_emacs_command(key: &TerminalKey) -> Option<EmacsCopyCommand> {
    use EmacsCopyCommand as C;
    let (ch, ctrl, alt) = emacs_key_parts(key)?;
    if ctrl && alt {
        return None;
    }
    if ctrl {
        return match ch.to_ascii_lowercase() {
            // Ctrl+Space arrives as NUL (`@`) on terminals without a keyboard protocol.
            ' ' | '@' | '2' => Some(C::BeginSelection),
            'f' => Some(C::CursorRight),
            'b' => Some(C::CursorLeft),
            'n' => Some(C::CursorDown),
            'p' => Some(C::CursorUp),
            'a' => Some(C::LineStart),
            'e' => Some(C::LineEnd),
            'v' => Some(C::PageDown),
            's' => Some(C::SearchForward),
            'r' => Some(C::SearchBackward),
            'w' => Some(C::CopyAndExit),
            'g' => Some(C::ClearSelection),
            _ => None,
        };
    }
    if alt {
        return match ch {
            'v' => Some(C::PageUp),
            'f' => Some(C::NextWordEnd),
            'b' => Some(C::PreviousWordStart),
            'w' => Some(C::CopyAndExit),
            'm' => Some(C::FirstNonBlank),
            '<' => Some(C::HistoryTop),
            '>' => Some(C::HistoryBottom),
            '{' => Some(C::PreviousParagraph),
            '}' => Some(C::NextParagraph),
            _ => None,
        };
    }
    match ch {
        'q' => Some(C::Cancel),
        'n' => Some(C::SearchAgain),
        'N' => Some(C::SearchReverse),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(ch: char, modifiers: KeyModifiers) -> TerminalKey {
        TerminalKey::new(KeyCode::Char(ch), modifiers)
    }

    #[test]
    fn emacs_movement_and_paging_match_tmux() {
        use EmacsCopyCommand as C;
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        for (k, want) in [
            (key('f', ctrl), C::CursorRight),
            (key('b', ctrl), C::CursorLeft),
            (key('n', ctrl), C::CursorDown),
            (key('p', ctrl), C::CursorUp),
            (key('a', ctrl), C::LineStart),
            (key('e', ctrl), C::LineEnd),
            (key('v', ctrl), C::PageDown),
            (key('v', alt), C::PageUp),
            (key('f', alt), C::NextWordEnd),
            (key('b', alt), C::PreviousWordStart),
            (key('m', alt), C::FirstNonBlank),
            (key('<', alt | KeyModifiers::SHIFT), C::HistoryTop),
            (key('>', alt | KeyModifiers::SHIFT), C::HistoryBottom),
            (key('{', alt | KeyModifiers::SHIFT), C::PreviousParagraph),
            (key('}', alt | KeyModifiers::SHIFT), C::NextParagraph),
        ] {
            assert_eq!(copy_mode_emacs_command(&k), Some(want), "{k:?}");
        }
    }

    #[test]
    fn emacs_selection_search_and_exit_match_tmux() {
        use EmacsCopyCommand as C;
        let ctrl = KeyModifiers::CONTROL;
        let none = KeyModifiers::empty();
        for (k, want) in [
            (key(' ', ctrl), C::BeginSelection),
            (key('@', ctrl), C::BeginSelection),
            (key('w', ctrl), C::CopyAndExit),
            (key('w', KeyModifiers::ALT), C::CopyAndExit),
            (key('g', ctrl), C::ClearSelection),
            (key('s', ctrl), C::SearchForward),
            (key('r', ctrl), C::SearchBackward),
            (key('n', none), C::SearchAgain),
            (key('N', KeyModifiers::SHIFT), C::SearchReverse),
            (key('q', none), C::Cancel),
        ] {
            assert_eq!(copy_mode_emacs_command(&k), Some(want), "{k:?}");
        }
    }

    #[test]
    fn emacs_table_ignores_vi_keys_and_unbound_chords() {
        let none = KeyModifiers::empty();
        for k in [
            key('j', none),
            key('y', none),
            key('v', none),
            key('h', none),
            key('x', KeyModifiers::CONTROL),
            key('x', KeyModifiers::ALT),
            key('f', KeyModifiers::CONTROL | KeyModifiers::ALT),
        ] {
            assert_eq!(copy_mode_emacs_command(&k), None, "{k:?}");
        }
    }
}
