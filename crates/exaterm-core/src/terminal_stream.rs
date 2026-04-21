use std::time::{Duration, Instant};

const PAINT_CONSOLIDATE_SETTLE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedLine {
    pub text: String,
    pub overwrite_count: usize,
}

#[derive(Debug, Default)]
pub struct PaintedLineTracker {
    current_line: String,
    last_emitted: Option<String>,
}

#[derive(Debug, Default)]
pub struct PaintConsolidator {
    pending: Option<String>,
    last_update_at: Option<Instant>,
    last_emitted: Option<String>,
}

#[derive(Debug, Default)]
pub struct TerminalStreamProcessor {
    carry: String,
    overwrite_count: usize,
    in_alternate_screen: bool,
    painted_line_tracker: PaintedLineTracker,
    paint_consolidator: PaintConsolidator,
}

#[derive(Debug, Default)]
pub struct StreamUpdate {
    pub semantic_lines: Vec<String>,
    pub painted_line: Option<String>,
}

impl StreamUpdate {
    pub fn is_empty(&self) -> bool {
        self.semantic_lines.is_empty() && self.painted_line.is_none()
    }
}

impl TerminalStreamProcessor {
    pub fn ingest(&mut self, chunk: &[u8]) -> StreamUpdate {
        let was_in_alt = self.in_alternate_screen;
        self.in_alternate_screen = detect_alternate_screen(chunk, self.in_alternate_screen);

        // Just exited alternate screen — clear stale parser state so the
        // partial carry from before the TUI doesn't contaminate new output.
        if was_in_alt && !self.in_alternate_screen {
            self.carry.clear();
            self.overwrite_count = 0;
            self.painted_line_tracker = PaintedLineTracker::default();
        }

        let semantic_lines = decode_chunk(chunk, &mut self.carry, &mut self.overwrite_count)
            .into_iter()
            .map(|line| line.text)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        let painted_line = self
            .painted_line_tracker
            .ingest(chunk)
            .map(|line| {
                self.paint_consolidator.ingest(line);
            })
            .and_then(|_| self.paint_consolidator.maybe_emit())
            .or_else(|| self.paint_consolidator.maybe_emit());

        StreamUpdate {
            semantic_lines,
            painted_line,
        }
    }

    /// Whether the processor believes the terminal is in alternate screen mode.
    pub fn in_alternate_screen(&self) -> bool {
        self.in_alternate_screen
    }
}

/// Scan a chunk for DEC Private Mode sequences that switch the alternate screen buffer.
///
/// Recognized sequences:
/// - `ESC [ ? 1049 h` — switch to alternate screen (smcup)
/// - `ESC [ ? 1049 l` — switch to primary screen (rmcup)
/// - `ESC [ ? 47 h` / `ESC [ ? 47 l` — older alternate screen variant
/// - `ESC [ ? 1047 h` / `ESC [ ? 1047 l` — another variant
///
/// Returns the updated alternate-screen state. The last transition in the chunk wins.
fn detect_alternate_screen(chunk: &[u8], mut in_alt: bool) -> bool {
    let mut i = 0;
    while i < chunk.len() {
        if chunk[i] == 0x1b && i + 1 < chunk.len() && chunk[i + 1] == b'[' {
            i += 2; // skip ESC [
            if i < chunk.len() && chunk[i] == b'?' {
                i += 1; // skip ?
                        // Parse numeric parameter.
                let num_start = i;
                while i < chunk.len() && chunk[i].is_ascii_digit() {
                    i += 1;
                }
                if i > num_start && i < chunk.len() {
                    let param = &chunk[num_start..i];
                    let final_byte = chunk[i];
                    i += 1;
                    if param == b"1049" || param == b"47" || param == b"1047" {
                        match final_byte {
                            b'h' => in_alt = true,
                            b'l' => in_alt = false,
                            _ => {}
                        }
                    }
                }
            } else {
                // Skip non-private CSI sequence.
                while i < chunk.len() {
                    let byte = chunk[i];
                    i += 1;
                    if (byte as char).is_ascii_alphabetic() || byte == b'~' {
                        break;
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    in_alt
}

pub fn decode_chunk(
    chunk: &[u8],
    carry: &mut String,
    overwrite_count: &mut usize,
) -> Vec<DecodedLine> {
    let mut lines = Vec::new();
    let mut index = 0usize;
    let mut printable = Vec::new();

    while index < chunk.len() {
        let flush_printable = |printable: &mut Vec<u8>, carry: &mut String| {
            if !printable.is_empty() {
                carry.push_str(&String::from_utf8_lossy(printable));
                printable.clear();
            }
        };

        match chunk[index] {
            0x1b => {
                flush_printable(&mut printable, carry);
                index += 1;
                if index < chunk.len() {
                    match chunk[index] {
                        b'[' => {
                            index += 1;
                            let start = index;
                            while index < chunk.len() {
                                let byte = chunk[index];
                                index += 1;
                                if (byte as char).is_ascii_alphabetic() || byte == b'~' {
                                    if csi_implies_rewrite(&chunk[start..index]) {
                                        carry.clear();
                                        *overwrite_count += 1;
                                    }
                                    break;
                                }
                            }
                        }
                        b']' => {
                            index += 1;
                            while index < chunk.len() {
                                match chunk[index] {
                                    0x07 => {
                                        index += 1;
                                        break;
                                    }
                                    0x1b if index + 1 < chunk.len()
                                        && chunk[index + 1] == b'\\' =>
                                    {
                                        index += 2;
                                        break;
                                    }
                                    _ => index += 1,
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            b'\r' => {
                flush_printable(&mut printable, carry);
                if index + 1 < chunk.len() && chunk[index + 1] == b'\n' {
                    if !carry.is_empty() {
                        lines.push(DecodedLine {
                            text: carry.trim_end().to_string(),
                            overwrite_count: *overwrite_count,
                        });
                        carry.clear();
                        *overwrite_count = 0;
                    }
                    index += 2;
                } else {
                    carry.clear();
                    *overwrite_count += 1;
                    index += 1;
                }
            }
            b'\n' => {
                flush_printable(&mut printable, carry);
                if !carry.is_empty() {
                    lines.push(DecodedLine {
                        text: carry.trim_end().to_string(),
                        overwrite_count: *overwrite_count,
                    });
                    carry.clear();
                    *overwrite_count = 0;
                }
                index += 1;
            }
            0x08 => {
                flush_printable(&mut printable, carry);
                carry.pop();
                index += 1;
            }
            byte if !byte.is_ascii_control() || byte == b'\t' => {
                printable.push(byte);
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    if !printable.is_empty() {
        carry.push_str(&String::from_utf8_lossy(&printable));
    }

    lines
}

pub fn csi_implies_rewrite(sequence: &[u8]) -> bool {
    let Some(final_byte) = sequence.last().copied() else {
        return false;
    };

    matches!(final_byte, b'G' | b'H' | b'f' | b'J' | b'K' | b'P' | b'X')
}

impl PaintedLineTracker {
    pub fn ingest(&mut self, chunk: &[u8]) -> Option<String> {
        let mut index = 0usize;
        let mut printable = Vec::new();
        let mut candidate = None::<String>;

        let flush_printable =
            |printable: &mut Vec<u8>, current_line: &mut String, candidate: &mut Option<String>| {
                if !printable.is_empty() {
                    current_line.push_str(&String::from_utf8_lossy(printable));
                    printable.clear();
                    let trimmed = current_line.trim();
                    if !trimmed.is_empty() {
                        *candidate = Some(trimmed.to_string());
                    }
                }
            };

        while index < chunk.len() {
            match chunk[index] {
                0x1b => {
                    flush_printable(&mut printable, &mut self.current_line, &mut candidate);
                    index += 1;
                    if index < chunk.len() {
                        match chunk[index] {
                            b'[' => {
                                index += 1;
                                let start = index;
                                while index < chunk.len() {
                                    let byte = chunk[index];
                                    index += 1;
                                    if (byte as char).is_ascii_alphabetic() || byte == b'~' {
                                        if csi_implies_rewrite(&chunk[start..index]) {
                                            self.current_line.clear();
                                        }
                                        break;
                                    }
                                }
                            }
                            b']' => {
                                index += 1;
                                while index < chunk.len() {
                                    match chunk[index] {
                                        0x07 => {
                                            index += 1;
                                            break;
                                        }
                                        0x1b if index + 1 < chunk.len()
                                            && chunk[index + 1] == b'\\' =>
                                        {
                                            index += 2;
                                            break;
                                        }
                                        _ => index += 1,
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                b'\r' => {
                    flush_printable(&mut printable, &mut self.current_line, &mut candidate);
                    self.current_line.clear();
                    if index + 1 < chunk.len() && chunk[index + 1] == b'\n' {
                        index += 2;
                    } else {
                        index += 1;
                    }
                }
                b'\n' => {
                    flush_printable(&mut printable, &mut self.current_line, &mut candidate);
                    self.current_line.clear();
                    index += 1;
                }
                0x08 => {
                    flush_printable(&mut printable, &mut self.current_line, &mut candidate);
                    self.current_line.pop();
                    let trimmed = self.current_line.trim();
                    if !trimmed.is_empty() {
                        candidate = Some(trimmed.to_string());
                    }
                    index += 1;
                }
                byte if !byte.is_ascii_control() || byte == b'\t' => {
                    printable.push(byte);
                    index += 1;
                }
                _ => {
                    index += 1;
                }
            }
        }

        flush_printable(&mut printable, &mut self.current_line, &mut candidate);

        match candidate {
            Some(line) if self.last_emitted.as_ref() != Some(&line) => {
                self.last_emitted = Some(line.clone());
                Some(line)
            }
            _ => None,
        }
    }
}

impl PaintConsolidator {
    pub fn ingest(&mut self, line: String) {
        self.ingest_at(line, Instant::now());
    }

    pub fn maybe_emit(&mut self) -> Option<String> {
        self.maybe_emit_at(Instant::now())
    }

    fn ingest_at(&mut self, line: String, now: Instant) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        self.pending = Some(match self.pending.take() {
            Some(existing) => merge_paint_lines(&existing, trimmed),
            None => trimmed.to_string(),
        });
        self.last_update_at = Some(now);
    }

    fn maybe_emit_at(&mut self, now: Instant) -> Option<String> {
        let pending = self.pending.clone()?;
        let settled = self.last_update_at.is_some_and(|last_update_at| {
            now.duration_since(last_update_at) >= PAINT_CONSOLIDATE_SETTLE
        });
        if !settled {
            return None;
        }
        if !looks_consolidated_worthy(&pending) {
            return None;
        }
        if self.last_emitted.as_ref() == Some(&pending) {
            return None;
        }
        self.last_emitted = Some(pending.clone());
        Some(pending)
    }
}

pub fn merge_paint_lines(existing: &str, incoming: &str) -> String {
    if incoming == existing {
        return existing.to_string();
    }
    if incoming.chars().all(|ch| ch.is_ascii_digit()) && looks_wordish(existing) {
        return format!("{existing} {incoming}");
    }
    if is_tiny_paint_fragment(incoming) {
        return existing.to_string();
    }
    if incoming.len() >= existing.len() && incoming.starts_with(existing) {
        return incoming.to_string();
    }
    if existing.len() >= incoming.len()
        && (existing.starts_with(incoming) || existing.contains(incoming))
    {
        return existing.to_string();
    }
    if incoming.len() > existing.len() && incoming.contains(existing) {
        return incoming.to_string();
    }
    incoming.to_string()
}

fn is_tiny_paint_fragment(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed == "•" {
        return true;
    }
    let visible = trimmed.chars().filter(|ch| !ch.is_whitespace()).count();
    let alpha = trimmed.chars().filter(|ch| ch.is_alphabetic()).count();
    visible <= 2 || (visible <= 4 && alpha <= 1)
}

fn looks_wordish(text: &str) -> bool {
    let alpha = text.chars().filter(|ch| ch.is_alphabetic()).count();
    alpha >= 4
}

fn looks_consolidated_worthy(text: &str) -> bool {
    let visible = text.chars().filter(|ch| !ch.is_whitespace()).count();
    let alpha = text.chars().filter(|ch| ch.is_alphabetic()).count();
    visible >= 4 || alpha >= 3
}

#[cfg(test)]
mod tests {
    use super::{
        csi_implies_rewrite, decode_chunk, detect_alternate_screen, merge_paint_lines, DecodedLine,
        PaintConsolidator, PaintedLineTracker, TerminalStreamProcessor,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn decodes_chunks_into_lines() {
        let mut carry = String::new();
        let mut overwrite_count = 0usize;
        let lines = decode_chunk(b"hello\r\nworld\npartial", &mut carry, &mut overwrite_count);
        assert_eq!(
            lines,
            vec![
                DecodedLine {
                    text: "hello".to_string(),
                    overwrite_count: 0,
                },
                DecodedLine {
                    text: "world".to_string(),
                    overwrite_count: 0,
                }
            ]
        );
        assert_eq!(carry, "partial");
    }

    #[test]
    fn carriage_return_overwrites_in_place_status_updates() {
        let mut carry = String::new();
        let mut overwrite_count = 0usize;
        let lines = decode_chunk(
            b"Working 1\rWorking 2\rWorking 3\nsteady line\n",
            &mut carry,
            &mut overwrite_count,
        );
        assert_eq!(
            lines,
            vec![
                DecodedLine {
                    text: "Working 3".to_string(),
                    overwrite_count: 2,
                },
                DecodedLine {
                    text: "steady line".to_string(),
                    overwrite_count: 0,
                }
            ]
        );
        assert!(carry.is_empty());
    }

    #[test]
    fn rewrite_like_csi_sequences_increment_overwrite_count() {
        let mut carry = String::new();
        let mut overwrite_count = 0usize;
        let lines = decode_chunk(b"alpha\x1b[2Kbeta\n", &mut carry, &mut overwrite_count);
        assert_eq!(
            lines,
            vec![DecodedLine {
                text: "beta".to_string(),
                overwrite_count: 1,
            }]
        );
    }

    #[test]
    fn recognizes_rewrite_like_csi_ops() {
        assert!(csi_implies_rewrite(b"2K"));
        assert!(csi_implies_rewrite(b"1G"));
        assert!(csi_implies_rewrite(b"2J"));
        assert!(!csi_implies_rewrite(b"31m"));
    }

    #[test]
    fn painted_line_tracker_follows_overwrites() {
        let mut tracker = PaintedLineTracker::default();
        let painted = tracker
            .ingest(b"Working 1\rWorking 2\rWorking 3")
            .expect("painted line should update");
        assert_eq!(painted, "Working 3");
    }

    #[test]
    fn painted_line_tracker_follows_rewrite_like_csi() {
        let mut tracker = PaintedLineTracker::default();
        let painted = tracker
            .ingest(b"alpha\x1b[2Kbeta")
            .expect("painted line should update");
        assert_eq!(painted, "beta");
    }

    #[test]
    fn consolidator_merges_prefix_fragments() {
        assert_eq!(merge_paint_lines("Work", "Worki"), "Worki");
        assert_eq!(merge_paint_lines("Working", "orking"), "Working");
        assert_eq!(merge_paint_lines("Working", "1"), "Working 1");
    }

    #[test]
    fn consolidator_emits_settled_snapshots() {
        let mut consolidator = PaintConsolidator::default();
        let now = Instant::now();
        consolidator.ingest_at("W".into(), now);
        consolidator.ingest_at("Wo".into(), now + Duration::from_millis(10));
        consolidator.ingest_at("Wor".into(), now + Duration::from_millis(20));
        consolidator.ingest_at("Working".into(), now + Duration::from_millis(40));
        let painted = consolidator
            .maybe_emit_at(now + Duration::from_millis(250))
            .expect("settled snapshot should emit");
        assert_eq!(painted, "Working");
    }

    #[test]
    fn consolidator_allows_new_sentence_to_replace_status() {
        let mut consolidator = PaintConsolidator::default();
        let now = Instant::now();
        consolidator.ingest_at("Working".into(), now);
        consolidator.ingest_at(
            "Reviewing the current repository state first".into(),
            now + Duration::from_millis(40),
        );
        let painted = consolidator
            .maybe_emit_at(now + Duration::from_millis(250))
            .expect("sentence snapshot should emit");
        assert_eq!(painted, "Reviewing the current repository state first");
    }

    // ---- alternate screen detection ----

    #[test]
    fn detect_alternate_screen_smcup() {
        // ESC [ ? 1049 h enters alternate screen.
        assert!(detect_alternate_screen(b"\x1b[?1049h", false));
    }

    #[test]
    fn detect_alternate_screen_rmcup() {
        // ESC [ ? 1049 l exits alternate screen.
        assert!(!detect_alternate_screen(b"\x1b[?1049l", true));
    }

    #[test]
    fn detect_alternate_screen_ignores_unrelated_sequences() {
        // ESC [ ? 25 h (show cursor) should not change state.
        assert!(!detect_alternate_screen(b"\x1b[?25h", false));
        assert!(detect_alternate_screen(b"\x1b[?25h", true));
    }

    #[test]
    fn detect_alternate_screen_variant_47() {
        assert!(detect_alternate_screen(b"\x1b[?47h", false));
        assert!(!detect_alternate_screen(b"\x1b[?47l", true));
    }

    #[test]
    fn detect_alternate_screen_variant_1047() {
        assert!(detect_alternate_screen(b"\x1b[?1047h", false));
        assert!(!detect_alternate_screen(b"\x1b[?1047l", true));
    }

    #[test]
    fn detect_alternate_screen_last_transition_wins() {
        // Both enter and exit in one chunk — last one wins.
        assert!(!detect_alternate_screen(b"\x1b[?1049h\x1b[?1049l", false));
        assert!(detect_alternate_screen(b"\x1b[?1049l\x1b[?1049h", true));
    }

    #[test]
    fn detect_alternate_screen_mixed_with_normal_output() {
        let chunk = b"hello\r\n\x1b[?1049hworld";
        assert!(detect_alternate_screen(chunk, false));
    }

    // ---- processor alternate screen integration ----

    #[test]
    fn processor_tracks_alternate_screen_state() {
        let mut proc = TerminalStreamProcessor::default();

        let update = proc.ingest(b"normal line\n");
        assert_eq!(update.semantic_lines, vec!["normal line"]);
        assert!(!proc.in_alternate_screen());

        // Enter alternate screen — state is tracked but output still flows.
        let _ = proc.ingest(b"\x1b[?1049h");
        assert!(proc.in_alternate_screen());

        // Exit alternate screen.
        let _ = proc.ingest(b"\x1b[?1049l");
        assert!(!proc.in_alternate_screen());

        // Subsequent normal output works.
        let update = proc.ingest(b"back to normal\n");
        assert_eq!(update.semantic_lines, vec!["back to normal"]);
    }

    #[test]
    fn processor_clears_carry_on_alternate_screen_exit() {
        let mut proc = TerminalStreamProcessor::default();

        // Start some partial output.
        let _ = proc.ingest(b"partial");

        // Enter alternate screen — TUI runs.
        let _ = proc.ingest(b"\x1b[?1049h");

        // Exit alternate screen.
        let _ = proc.ingest(b"\x1b[?1049l");

        // New output after TUI exit should not carry stale partial data.
        let update = proc.ingest(b"fresh line\n");
        assert_eq!(update.semantic_lines, vec!["fresh line"]);
    }
}
