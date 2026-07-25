use std::sync::OnceLock;

use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionChunkPayload {
    pub meeting_id: String,
    pub meeting_title: String,
    pub speaker: String,
    pub text: String,
    pub is_final: bool,
    pub timestamp: i64,
    pub chunk_index: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingScheduledPayload {
    pub meeting_id: String,
    pub title: String,
    pub scheduled_at: i64,
    pub starts_in_minutes: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingStartedPayload {
    pub meeting_id: String,
    pub title: String,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingEndedPayload {
    pub meeting_id: String,
    pub ended_at: i64,
    pub has_summary: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingPausedPayload {
    pub meeting_id: String,
    pub paused_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingResumedPayload {
    pub meeting_id: String,
    pub resumed_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum WsMessage {
    #[serde(rename = "transcription.chunk")]
    TranscriptionChunk(TranscriptionChunkPayload),
    #[serde(rename = "meeting.scheduled")]
    MeetingScheduled(MeetingScheduledPayload),
    #[serde(rename = "meeting.started")]
    MeetingStarted(MeetingStartedPayload),
    #[serde(rename = "meeting.ended")]
    MeetingEnded(MeetingEndedPayload),
    #[serde(rename = "meeting.paused")]
    MeetingPaused(MeetingPausedPayload),
    #[serde(rename = "meeting.resumed")]
    MeetingResumed(MeetingResumedPayload),
}

pub struct Broadcaster {
    tx: broadcast::Sender<WsMessage>,
}

impl Broadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn broadcast(&self, msg: WsMessage) {
        let _ = self.tx.send(msg);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsMessage> {
        self.tx.subscribe()
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new(1024)
    }
}

static GLOBAL_BROADCASTER: OnceLock<Broadcaster> = OnceLock::new();

pub fn initialize_broadcaster() {
    let _ = GLOBAL_BROADCASTER.set(Broadcaster::default());
}

pub fn get_broadcaster() -> Option<&'static Broadcaster> {
    GLOBAL_BROADCASTER.get()
}
