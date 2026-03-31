use std::io::Write;

/// Detect if running inside tmux.
pub fn in_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

/// Escape components for Kitty APC sequences.
/// Returns (start, escape, end) where:
/// - `start`: prefix before each APC sequence
/// - `escape`: the ESC byte (doubled inside tmux DCS passthrough)
/// - `end`: suffix after each APC sequence
fn kitty_escape(tmux: bool) -> (&'static str, &'static str, &'static str) {
    if tmux {
        ("\x1bPtmux;", "\x1b\x1b", "\x1b\\")
    } else {
        ("", "\x1b", "")
    }
}

/// Write a complete Kitty APC command to stdout.
/// Wraps in DCS passthrough when inside tmux.
pub fn write_kitty_cmd(stdout: &mut impl Write, params: &str) -> std::io::Result<()> {
    write_kitty_cmd_inner(stdout, params, in_tmux())
}

fn write_kitty_cmd_inner(stdout: &mut impl Write, params: &str, tmux: bool) -> std::io::Result<()> {
    let (start, esc, end) = kitty_escape(tmux);
    write!(stdout, "{start}{esc}_G{params}{esc}\\{end}")
}

/// Write a Kitty APC command with a data payload (base64 chunk).
pub fn write_kitty_cmd_with_data(stdout: &mut impl Write, params: &str, data: &[u8]) -> std::io::Result<()> {
    write_kitty_cmd_with_data_inner(stdout, params, data, in_tmux())
}

fn write_kitty_cmd_with_data_inner(stdout: &mut impl Write, params: &str, data: &[u8], tmux: bool) -> std::io::Result<()> {
    let (start, esc, end) = kitty_escape(tmux);
    write!(stdout, "{start}{esc}_G{params};")?;
    stdout.write_all(data)?;
    write!(stdout, "{esc}\\{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kitty_cmd_direct() {
        let mut buf = Vec::new();
        write_kitty_cmd_inner(&mut buf, "a=d,d=I,i=31,q=2", false).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "\x1b_Ga=d,d=I,i=31,q=2\x1b\\"
        );
    }

    #[test]
    fn kitty_cmd_with_data_direct() {
        let mut buf = Vec::new();
        write_kitty_cmd_with_data_inner(&mut buf, "m=1", b"AQID", false).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "\x1b_Gm=1;AQID\x1b\\"
        );
    }

    #[test]
    fn kitty_cmd_tmux_wrapped() {
        let mut buf = Vec::new();
        write_kitty_cmd_inner(&mut buf, "a=d,d=I,i=31,q=2", true).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "\x1bPtmux;\x1b\x1b_Ga=d,d=I,i=31,q=2\x1b\x1b\\\x1b\\"
        );
    }

    #[test]
    fn kitty_cmd_with_data_tmux_wrapped() {
        let mut buf = Vec::new();
        write_kitty_cmd_with_data_inner(&mut buf, "m=1", b"AQID", true).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "\x1bPtmux;\x1b\x1b_Gm=1;AQID\x1b\x1b\\\x1b\\"
        );
    }
}
