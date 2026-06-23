#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptRowFixture {
    pub meeting_id: &'static str,
    pub row_id: &'static str,
    pub sequence: u32,
    pub speaker: &'static str,
    pub text: &'static str,
}

pub const MEETING_IDS: [&str; 4] = [
    "meeting-2026-06-01-standup",
    "meeting-2026-06-02-design-review",
    "meeting-2026-06-03-launch-planning",
    "meeting-2026-06-04-localization-sync",
];

pub const DUPLICATE_MEETING_IDS: [&str; 5] = [
    "meeting-2026-06-01-standup",
    "meeting-2026-06-02-design-review",
    "meeting-2026-06-01-standup",
    "meeting-2026-06-03-launch-planning",
    "meeting-2026-06-02-design-review",
];

pub const ORDERED_TRANSCRIPT_ROWS: [TranscriptRowFixture; 4] = [
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0001",
        sequence: 1,
        speaker: "Ava",
        text: "Kickoff complete; the next step is approvals.",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0002",
        sequence: 2,
        speaker: "Ben",
        text: "次のアクションは、UI copy を確認することです。",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0003",
        sequence: 3,
        speaker: "Choi",
        text: "회의 메모를 정리한 뒤 공유하겠습니다.",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0004",
        sequence: 4,
        speaker: "Dina",
        text: "Résumé updated — gracias por la revisión. ✅",
    },
];

pub const OUT_OF_ORDER_TRANSCRIPT_ROWS: [TranscriptRowFixture; 4] = [
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0003",
        sequence: 3,
        speaker: "Choi",
        text: "회의 메모를 정리한 뒤 공유하겠습니다.",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0001",
        sequence: 1,
        speaker: "Ava",
        text: "Kickoff complete; the next step is approvals.",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0004",
        sequence: 4,
        speaker: "Dina",
        text: "Résumé updated — gracias por la revisión. ✅",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-02-design-review",
        row_id: "row-0002",
        sequence: 2,
        speaker: "Ben",
        text: "次のアクションは、UI copy を確認することです。",
    },
];

pub const DUPLICATE_TRANSCRIPT_ROW_IDS: [TranscriptRowFixture; 3] = [
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-04-localization-sync",
        row_id: "row-dup-1",
        sequence: 1,
        speaker: "Iris",
        text: "Initial import succeeded.",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-04-localization-sync",
        row_id: "row-dup-1",
        sequence: 2,
        speaker: "Iris",
        text: "Initial import succeeded.",
    },
    TranscriptRowFixture {
        meeting_id: "meeting-2026-06-04-localization-sync",
        row_id: "row-dup-2",
        sequence: 3,
        speaker: "Jules",
        text: "Fallback row for dedupe coverage.",
    },
];

pub fn ordered_transcript_rows() -> &'static [TranscriptRowFixture] {
    &ORDERED_TRANSCRIPT_ROWS
}

pub fn out_of_order_transcript_rows() -> &'static [TranscriptRowFixture] {
    &OUT_OF_ORDER_TRANSCRIPT_ROWS
}

pub fn duplicate_transcript_row_ids() -> &'static [TranscriptRowFixture] {
    &DUPLICATE_TRANSCRIPT_ROW_IDS
}

pub fn meeting_ids() -> &'static [&'static str] {
    &MEETING_IDS
}

pub fn duplicate_meeting_ids() -> &'static [&'static str] {
    &DUPLICATE_MEETING_IDS
}

pub fn is_sequence_ordered(rows: &[TranscriptRowFixture]) -> bool {
    rows.windows(2)
        .all(|pair| pair[0].sequence <= pair[1].sequence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_rows_are_sorted_by_sequence() {
        assert!(is_sequence_ordered(ordered_transcript_rows()));
    }

    #[test]
    fn out_of_order_rows_are_not_sorted_by_sequence() {
        assert!(!is_sequence_ordered(out_of_order_transcript_rows()));
    }

    #[test]
    fn unicode_rows_keep_non_ascii_text() {
        assert!(ordered_transcript_rows()
            .iter()
            .any(|row| !row.text.is_ascii()));
    }

    #[test]
    fn duplicate_meeting_ids_are_present() {
        let duplicates = duplicate_meeting_ids()
            .iter()
            .filter(|id| **id == "meeting-2026-06-01-standup")
            .count();

        assert!(duplicates > 1);
    }

    #[test]
    fn duplicate_row_ids_are_present() {
        let duplicates = duplicate_transcript_row_ids()
            .iter()
            .filter(|row| row.row_id == "row-dup-1")
            .count();

        assert!(duplicates > 1);
    }
}
