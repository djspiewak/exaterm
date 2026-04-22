pub struct HeadlessTerminal {
    parser: vt100::Parser,
}

impl HeadlessTerminal {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
        }
    }

    pub fn ingest(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.set_size(rows, cols);
    }

    /// Returns up to `max_rows` trailing rows of the visible screen, right-trimmed,
    /// with leading empty rows dropped.
    pub fn rendered_lines(&self, max_rows: usize) -> Vec<String> {
        let screen = self.parser.screen();

        // `contents()` returns plain text with rows separated by newlines.
        let contents = screen.contents();
        let mut all: Vec<String> = contents
            .lines()
            .map(|l| l.trim_end().to_string())
            .collect();

        // Drop leading empty rows to show only meaningful content.
        let first_nonempty = all.iter().position(|l| !l.is_empty()).unwrap_or(all.len());
        if first_nonempty > 0 {
            all.drain(0..first_nonempty);
        }

        // Take the last `max_rows` rows from what remains.
        if all.len() > max_rows {
            let drop = all.len() - max_rows;
            all.drain(0..drop);
        }

        all
    }

    pub fn in_alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen()
    }
}

impl Default for HeadlessTerminal {
    fn default() -> Self {
        Self::new(24, 80)
    }
}

#[cfg(test)]
mod tests {
    use super::HeadlessTerminal;

    #[test]
    fn alt_screen_cup_paint_renders_as_text() {
        let mut term = HeadlessTerminal::new(24, 80);
        // Enter alt screen, clear, place HELLO at row 3 col 5, WORLD at row 5 col 1.
        term.ingest(b"\x1b[?1049h\x1b[2J\x1b[3;5HHELLO\x1b[5;1HWORLD");
        let lines = term.rendered_lines(24);
        assert!(
            lines.iter().any(|l| l.contains("HELLO")),
            "rendered_lines should contain HELLO; got: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("WORLD")),
            "rendered_lines should contain WORLD; got: {lines:?}"
        );
    }

    #[test]
    fn line_erased_by_el_does_not_appear() {
        let mut term = HeadlessTerminal::new(24, 80);
        // Write ABC, erase entire line, write XYZ on the same row.
        term.ingest(b"\x1b[1;1HABC\x1b[2K\x1b[1;1HXYZ");
        let lines = term.rendered_lines(24);
        let row = lines.iter().find(|l| l.contains("XYZ") || l.contains("ABC"));
        assert!(
            row.is_some_and(|l| l.contains("XYZ") && !l.contains("ABC")),
            "after EL, only XYZ should remain; got: {lines:?}"
        );
    }

    #[test]
    fn cooked_echo_preserves_prompt_and_output() {
        let mut term = HeadlessTerminal::new(24, 80);
        // Simulate cooked echo: prompt, user input, command output.
        term.ingest(b"$ echo hello\r\nhello\r\n$ ");
        let lines = term.rendered_lines(24);
        let text = lines.join("\n");
        assert!(text.contains("echo hello"), "prompt echo missing; got: {text:?}");
        assert!(text.contains("hello"), "output missing; got: {text:?}");
    }

    #[test]
    fn exiting_alt_screen_restores_primary_buffer() {
        let mut term = HeadlessTerminal::new(24, 80);
        // Write something to primary buffer first.
        term.ingest(b"primary content\r\n");
        // Enter alt screen and paint something different.
        term.ingest(b"\x1b[?1049h\x1b[2J\x1b[1;1HALT CONTENT");
        // Exit alt screen.
        term.ingest(b"\x1b[?1049l");
        let lines = term.rendered_lines(24);
        let text = lines.join("\n");
        assert!(
            !text.contains("ALT CONTENT"),
            "alt screen content should not appear after exit; got: {text:?}"
        );
    }

    #[test]
    fn resize_reflows_and_preserves_tail() {
        let mut term = HeadlessTerminal::new(24, 80);
        // Paint 30 lines of content.
        for i in 0..30u32 {
            term.ingest(format!("line {i:03}\r\n").as_bytes());
        }
        term.resize(24, 120);
        let lines = term.rendered_lines(24);
        assert!(
            !lines.is_empty(),
            "should have visible lines after resize; got: {lines:?}"
        );
        let text = lines.join("\n");
        // The most recent lines should still be visible.
        assert!(
            text.contains("line 029"),
            "most recent line should survive resize; got: {text:?}"
        );
    }
}
