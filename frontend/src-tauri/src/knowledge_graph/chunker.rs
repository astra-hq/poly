use std::collections::HashSet;

const DEFAULT_CHUNK_INTERVAL_SECS: f64 = 20.0;
const MIN_CHUNK_INTERVAL_SECS: f64 = 5.0;
const MAX_CHUNK_INTERVAL_SECS: f64 = 120.0;

/// A single transcript row input for chunking.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptRow {
    pub id: String,
    pub audio_start_time: Option<f64>,
    pub audio_end_time: Option<f64>,
    pub text: String,
}

/// A chunk of transcript text with time boundaries.
/// Untimed chunks (rows without audio_start_time) have `None` for both fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub text: String,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
}

/// Chunks transcript rows into fixed-duration time windows.
///
/// # Behaviour
/// - Rows with empty text are skipped.
/// - Duplicate row IDs keep only the first occurrence.
/// - Rows are sorted by `audio_start_time`; rows without a start time
///   are collected into a single trailing untimed chunk.
/// - Unicode text is passed through unchanged.
/// - The final partial chunk is always flushed.
pub struct TranscriptChunker {
    pub chunk_interval_secs: f64,
}

impl TranscriptChunker {
    /// Creates a new chunker. The interval is clamped to [5.0, 120.0].
    pub fn new(chunk_interval_secs: f64) -> Self {
        Self {
            chunk_interval_secs: chunk_interval_secs.clamp(MIN_CHUNK_INTERVAL_SECS, MAX_CHUNK_INTERVAL_SECS),
        }
    }

    /// Creates a chunker with the default interval of 20 seconds.
    pub fn default() -> Self {
        Self::new(DEFAULT_CHUNK_INTERVAL_SECS)
    }

    /// Accepts transcript rows and returns time-based chunks.
    pub fn chunk(&self, rows: Vec<TranscriptRow>) -> Vec<Chunk> {
        // 1. Filter out rows with empty text
        let rows: Vec<TranscriptRow> = rows
            .into_iter()
            .filter(|r| !r.text.is_empty())
            .collect();

        // 2. Deduplicate by ID — keep first occurrence only
        let mut seen: HashSet<String> = HashSet::new();
        let rows: Vec<TranscriptRow> = rows
            .into_iter()
            .filter(|r| seen.insert(r.id.clone()))
            .collect();

        // 3. Separate timed and untimed rows.
        //    A row is untimed when audio_start_time is None, NaN, or infinite.
        let (mut timed, untimed): (Vec<TranscriptRow>, Vec<TranscriptRow>) = rows
            .into_iter()
            .partition(|r| {
                r.audio_start_time
                    .map(|t| t.is_finite())
                    .unwrap_or(false)
            });

        // 4. Sort timed rows by audio_start_time (stable, ascending)
        timed.sort_by(|a, b| {
            a.audio_start_time
                .unwrap()
                .partial_cmp(&b.audio_start_time.unwrap())
                .expect("audio_start_time values are finite and comparable")
        });

        let mut chunks: Vec<Chunk> = Vec::new();

        // 5. Chunk timed rows into fixed-duration windows.
        //    Windows are aligned to the interval grid (floor of the earliest start).
        if let Some(first_start) = timed.first().map(|r| r.audio_start_time.unwrap()) {
            let interval = self.chunk_interval_secs;
            let grid_origin = (first_start / interval).floor() * interval;
            let mut current_window: Option<(i64, String)> = None; // (window_idx, text)

            for row in &timed {
                let start = row.audio_start_time.unwrap();
                let window_idx = ((start - grid_origin) / interval).floor() as i64;

                match current_window {
                    Some((idx, ref mut text)) if idx == window_idx => {
                        // Same window — accumulate
                        text.push(' ');
                        text.push_str(&row.text);
                    }
                    Some((idx, text)) => {
                        // New window — flush the previous chunk
                        let ws = grid_origin + (idx as f64) * interval;
                        let we = ws + interval;
                        chunks.push(Chunk {
                            text,
                            start_time: Some(ws),
                            end_time: Some(we),
                        });
                        // Start accumulating into the new window
                        current_window = Some((window_idx, String::new()));
                        // Now add this row to the new window
                        if let Some((_, ref mut t)) = current_window {
                            t.push_str(&row.text);
                        }
                    }
                    None => {
                        // First row — start the first window
                        let mut text = String::new();
                        text.push_str(&row.text);
                        current_window = Some((window_idx, text));
                    }
                }
            }

            // 6. Flush the final partial chunk
            if let Some((idx, text)) = current_window {
                let ws = grid_origin + (idx as f64) * interval;
                let we = ws + interval;
                chunks.push(Chunk {
                    text,
                    start_time: Some(ws),
                    end_time: Some(we),
                });
            }
        }

        // 7. Collect untimed rows into a single final chunk
        if !untimed.is_empty() {
            let text = untimed
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<&str>>()
                .join(" ");
            // Only emit if the joined text is non-empty (it will be, since we filtered empty rows)
            chunks.push(Chunk {
                text,
                start_time: None,
                end_time: None,
            });
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────

    fn row(id: &str, start: f64, end: f64, text: &str) -> TranscriptRow {
        TranscriptRow {
            id: id.to_string(),
            audio_start_time: Some(start),
            audio_end_time: Some(end),
            text: text.to_string(),
        }
    }

    fn untimed_row(id: &str, text: &str) -> TranscriptRow {
        TranscriptRow {
            id: id.to_string(),
            audio_start_time: None,
            audio_end_time: None,
            text: text.to_string(),
        }
    }

    fn chunk_texts(chunks: &[Chunk]) -> Vec<&str> {
        chunks.iter().map(|c| c.text.as_str()).collect()
    }

    // ── Default / clamping ───────────────────────────────────────

    #[test]
    fn default_chunker_uses_20_second_interval() {
        let chunker = TranscriptChunker::default();
        assert_eq!(chunker.chunk_interval_secs, 20.0);
    }

    #[test]
    fn interval_clamped_to_min_5() {
        let chunker = TranscriptChunker::new(2.0);
        assert_eq!(chunker.chunk_interval_secs, 5.0);
    }

    #[test]
    fn interval_clamped_to_max_120() {
        let chunker = TranscriptChunker::new(300.0);
        assert_eq!(chunker.chunk_interval_secs, 120.0);
    }

    #[test]
    fn interval_exactly_5_is_accepted() {
        let chunker = TranscriptChunker::new(5.0);
        assert_eq!(chunker.chunk_interval_secs, 5.0);
    }

    #[test]
    fn interval_exactly_120_is_accepted() {
        let chunker = TranscriptChunker::new(120.0);
        assert_eq!(chunker.chunk_interval_secs, 120.0);
    }

    // ── Empty input ──────────────────────────────────────────────

    #[test]
    fn empty_input_produces_zero_chunks() {
        let chunker = TranscriptChunker::default();
        assert!(chunker.chunk(Vec::new()).is_empty());
    }

    // ── Empty text skip ──────────────────────────────────────────

    #[test]
    fn empty_text_rows_are_skipped() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("a", 0.0, 1.0, "hello"),
            TranscriptRow {
                id: "b".into(),
                audio_start_time: Some(2.0),
                audio_end_time: Some(3.0),
                text: "".into(),
            },
            row("c", 4.0, 5.0, "world"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunk_texts(&chunks), vec!["hello world"]);
    }

    // ── Duplicate IDs ────────────────────────────────────────────

    #[test]
    fn duplicate_ids_keep_first_occurrence() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("dup", 0.0, 1.0, "first"),
            row("dup", 2.0, 3.0, "second"),
            row("unique", 4.0, 5.0, "third"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunk_texts(&chunks), vec!["first third"]);
    }

    // ── Out-of-order sort ───────────────────────────────────────

    #[test]
    fn out_of_order_rows_are_sorted_by_audio_start_time() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("c", 40.0, 41.0, "third"),
            row("a", 0.0, 1.0, "first"),
            row("b", 20.0, 21.0, "second"),
        ];
        let chunks = chunker.chunk(rows);
        // With interval 20, the windows are [0,20), [20,40), [40,60)
        // Row "a" (0.0) → window 0, text "first"
        // Row "b" (20.0) → window 1, text "second"
        // Row "c" (40.0) → window 2, text "third"
        assert_eq!(chunk_texts(&chunks), vec!["first", "second", "third"]);
    }

    // ── Unicode unchanged ────────────────────────────────────────

    #[test]
    fn unicode_text_is_preserved() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("a", 0.0, 1.0, "こんにちは"),
            row("b", 2.0, 3.0, "안녕하세요"),
            row("c", 4.0, 5.0, "Résumé — café ✅"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(
            chunk_texts(&chunks),
            vec!["こんにちは 안녕하세요 Résumé — café ✅"]
        );
    }

    // ── Basic chunking ───────────────────────────────────────────

    #[test]
    fn single_row_produces_one_chunk() {
        let chunker = TranscriptChunker::default();
        let rows = vec![row("a", 5.0, 10.0, "hello")];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello");
        assert_eq!(chunks[0].start_time, Some(0.0));
        assert_eq!(chunks[0].end_time, Some(20.0));
    }

    #[test]
    fn rows_within_same_window_merge_into_one_chunk() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("a", 0.0, 1.0, "alpha"),
            row("b", 5.0, 6.0, "bravo"),
            row("c", 15.0, 16.0, "charlie"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunk_texts(&chunks), vec!["alpha bravo charlie"]);
        assert_eq!(chunks[0].start_time, Some(0.0));
        assert_eq!(chunks[0].end_time, Some(20.0));
    }

    #[test]
    fn rows_across_windows_produce_multiple_chunks() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("a", 5.0, 6.0, "alpha"),
            row("b", 25.0, 26.0, "bravo"),
            row("c", 45.0, 46.0, "charlie"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "alpha");
        assert_eq!(chunks[0].start_time, Some(0.0));
        assert_eq!(chunks[1].text, "bravo");
        assert_eq!(chunks[1].start_time, Some(20.0));
        assert_eq!(chunks[2].text, "charlie");
        assert_eq!(chunks[2].start_time, Some(40.0));
    }

    #[test]
    fn grid_origin_floors_to_nearest_interval() {
        // First row starts at 35.0, interval is 20.
        // Grid origin: floor(35/20)*20 = 20
        // Window 0: [20, 40) → row at 35
        // Window 1: [40, 60) → row at 50
        // Window 2: [60, 80) → row at 60
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("a", 35.0, 36.0, "alpha"),  // window 0
            row("b", 50.0, 51.0, "bravo"),  // window 1
            row("c", 60.0, 61.0, "charlie"), // window 2
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "alpha");
        assert_eq!(chunks[0].start_time, Some(20.0));
        assert_eq!(chunks[0].end_time, Some(40.0));
        assert_eq!(chunks[1].text, "bravo");
        assert_eq!(chunks[1].start_time, Some(40.0));
        assert_eq!(chunks[1].end_time, Some(60.0));
        assert_eq!(chunks[2].text, "charlie");
        assert_eq!(chunks[2].start_time, Some(60.0));
        assert_eq!(chunks[2].end_time, Some(80.0));
    }

    // ── Final partial chunk flush ────────────────────────────────

    #[test]
    fn final_partial_chunk_is_flushed() {
        // Only one row in the first window — should still produce a chunk
        let chunker = TranscriptChunker::default();
        let rows = vec![row("a", 5.0, 6.0, "only")];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "only");
    }

    // ── Untimed rows ─────────────────────────────────────────────

    #[test]
    fn untimed_rows_become_final_chunk() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("a", 0.0, 1.0, "timed alpha"),
            untimed_row("b", "untimed bravo"),
            row("c", 2.0, 3.0, "timed charlie"),
            untimed_row("d", "untimed delta"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 2);
        // Timed rows in one chunk
        assert_eq!(chunks[0].text, "timed alpha timed charlie");
        assert!(chunks[0].start_time.is_some());
        // Untimed rows in final chunk
        assert_eq!(chunks[1].text, "untimed bravo untimed delta");
        assert_eq!(chunks[1].start_time, None);
        assert_eq!(chunks[1].end_time, None);
    }

    #[test]
    fn only_untimed_rows_produce_single_chunk() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            untimed_row("a", "foo"),
            untimed_row("b", "bar"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "foo bar");
        assert_eq!(chunks[0].start_time, None);
    }

    #[test]
    fn nan_start_time_treated_as_untimed() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            TranscriptRow {
                id: "n".into(),
                audio_start_time: Some(f64::NAN),
                audio_end_time: Some(5.0),
                text: "nan row".into(),
            },
            untimed_row("u", "untimed row"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "nan row untimed row");
        assert_eq!(chunks[0].start_time, None);
    }

    #[test]
    fn infinite_start_time_treated_as_untimed() {
        let chunker = TranscriptChunker::default();
        let rows = vec![
            TranscriptRow {
                id: "inf".into(),
                audio_start_time: Some(f64::INFINITY),
                audio_end_time: Some(5.0),
                text: "infinite row".into(),
            },
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_time, None);
    }

    // ── Gapped windows ───────────────────────────────────────────

    #[test]
    fn gaps_between_windows_skip_empty_chunks() {
        // Rows at 0, 5, and 50 with interval 20
        // Window 0 ([0,20)): rows at 0, 5
        // Window 1 ([20,40)): empty — no chunk emitted
        // Window 2 ([40,60)): row at 50
        let chunker = TranscriptChunker::default();
        let rows = vec![
            row("a", 0.0, 1.0, "first"),
            row("b", 5.0, 6.0, "second"),
            row("c", 50.0, 51.0, "third"),
        ];
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "first second");
        assert_eq!(chunks[0].start_time, Some(0.0));
        assert_eq!(chunks[0].end_time, Some(20.0));
        assert_eq!(chunks[1].text, "third");
        assert_eq!(chunks[1].start_time, Some(40.0));
        assert_eq!(chunks[1].end_time, Some(60.0));
    }

    // ── Stable output ────────────────────────────────────────────

    #[test]
    fn same_input_produces_identical_output() {
        let chunker = TranscriptChunker::default();
        let mk_rows = || {
            vec![
                row("a", 2.0, 3.0, "one"),
                row("b", 0.0, 1.0, "two"), // out of order
            ]
        };
        let chunks1 = chunker.chunk(mk_rows());
        let chunks2 = chunker.chunk(mk_rows());
        assert_eq!(chunks1, chunks2);
    }

    // ── Custom interval ──────────────────────────────────────────

    #[test]
    fn custom_10_second_interval_chunks_correctly() {
        let chunker = TranscriptChunker::new(10.0);
        let rows = vec![
            row("a", 0.0, 1.0, "alpha"),
            row("b", 5.0, 6.0, "bravo"),
            row("c", 12.0, 13.0, "charlie"),
            row("d", 22.0, 23.0, "delta"),
        ];
        // Windows: [0,10), [10,20), [20,30)
        let chunks = chunker.chunk(rows);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "alpha bravo");
        assert_eq!(chunks[0].start_time, Some(0.0));
        assert_eq!(chunks[0].end_time, Some(10.0));
        assert_eq!(chunks[1].text, "charlie");
        assert_eq!(chunks[1].start_time, Some(10.0));
        assert_eq!(chunks[1].end_time, Some(20.0));
        assert_eq!(chunks[2].text, "delta");
        assert_eq!(chunks[2].start_time, Some(20.0));
        assert_eq!(chunks[2].end_time, Some(30.0));
    }
}
