use std::collections::HashMap;

use flume::Sender;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
use tokio::sync::Mutex;

use crate::message::{build_message_sender, Message, MessageType, SenderType};

pub struct InputRouter {
    handles: Mutex<HashMap<usize, ChildStdin>>,
    names: Vec<String>,
    num_processes: usize,
    default_target: usize,
    message_tx: Sender<Message>,
}

/// The result of parsing an input line.
struct ParsedInput {
    /// The resolved process index to send to.
    target: usize,
    /// The payload to write to the child's stdin.
    payload: String,
    /// Display name for feedback messages.
    target_name: String,
}

impl InputRouter {
    pub fn new(
        names: Vec<String>,
        num_processes: usize,
        default_target: usize,
        message_tx: Sender<Message>,
    ) -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            names,
            num_processes,
            default_target,
            message_tx,
        }
    }

    /// Try to resolve a candidate string to a process index.
    /// Returns Some(index) if the candidate matches a name or is a valid index
    /// within 0..num_processes. Returns None otherwise.
    fn resolve_target(&self, candidate: &str) -> Option<usize> {
        // Try name match first
        if let Some(idx) = self.names.iter().position(|n| n == candidate) {
            return Some(idx);
        }
        // Fall back to index
        if let Ok(idx) = candidate.parse::<usize>() {
            if idx < self.num_processes {
                return Some(idx);
            }
        }
        None
    }

    /// Parse an input line into a target index and payload.
    fn parse_line(&self, line: &str) -> ParsedInput {
        if let Some(colon_pos) = line.find(':') {
            let candidate = &line[..colon_pos];
            if let Some(idx) = self.resolve_target(candidate) {
                let payload = line[colon_pos + 1..].to_string();
                let target_name = self.display_name(idx);
                return ParsedInput {
                    target: idx,
                    payload,
                    target_name,
                };
            }
        }
        // No colon or unresolved prefix — send whole line to default
        let target_name = self.display_name(self.default_target);
        ParsedInput {
            target: self.default_target,
            payload: line.to_string(),
            target_name,
        }
    }

    /// Get a display name for a process index.
    fn display_name(&self, index: usize) -> String {
        self.names
            .get(index)
            .cloned()
            .unwrap_or_else(|| index.to_string())
    }

    pub async fn register(&self, index: usize, stdin: ChildStdin) {
        self.handles.lock().await.insert(index, stdin);
    }

    pub async fn deregister(&self, index: usize) {
        self.handles.lock().await.remove(&index);
    }

    pub async fn route(&self, line: &str) {
        let parsed = self.parse_line(line);
        let mut handles = self.handles.lock().await;

        if handles.is_empty() {
            self.send_message("[mlti] No running processes, input discarded".to_string());
            return;
        }

        match handles.get_mut(&parsed.target) {
            Some(stdin) => {
                let data = format!("{}\n", parsed.payload);
                if let Err(_) = stdin.write_all(data.as_bytes()).await {
                    // Process died between lookup and write
                    drop(handles);
                    self.deregister(parsed.target).await;
                    self.send_message(format!(
                        "[mlti] Failed to send input to \"{}\" (process exited)",
                        parsed.target_name
                    ));
                    return;
                }
                if let Err(_) = stdin.flush().await {
                    drop(handles);
                    self.deregister(parsed.target).await;
                    self.send_message(format!(
                        "[mlti] Failed to send input to \"{}\" (process exited)",
                        parsed.target_name
                    ));
                    return;
                }
                self.send_message(format!(
                    "[mlti] -> {}: {}",
                    parsed.target_name, parsed.payload
                ));
            }
            None => {
                self.send_message(format!(
                    "[mlti] Unknown target \"{}\", input discarded",
                    parsed.target_name
                ));
            }
        }
    }

    fn send_message(&self, data: String) {
        self.message_tx
            .send(Message::new(
                MessageType::Text,
                Some("".to_string()),
                Some(data),
                None,
                build_message_sender(SenderType::Main, None, None),
            ))
            .expect("Could not send message on channel.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_router(names: Vec<&str>, num_processes: usize, default_target: usize) -> InputRouter {
        let (tx, _rx) = flume::unbounded();
        InputRouter::new(
            names.into_iter().map(String::from).collect(),
            num_processes,
            default_target,
            tx,
        )
    }

    // -- resolve_target --

    #[test]
    fn resolve_by_name() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("server"), Some(0));
        assert_eq!(router.resolve_target("worker"), Some(1));
    }

    #[test]
    fn resolve_by_index() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("0"), Some(0));
        assert_eq!(router.resolve_target("1"), Some(1));
    }

    #[test]
    fn resolve_name_takes_priority_over_index() {
        // Process named "1" should match by name (index 0), not by index 1
        let router = make_router(vec!["1", "worker"], 2, 0);
        assert_eq!(router.resolve_target("1"), Some(0));
    }

    #[test]
    fn resolve_unknown_name_returns_none() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("unknown"), None);
    }

    #[test]
    fn resolve_out_of_range_index_returns_none() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.resolve_target("99"), None);
    }

    // -- parse_line --

    #[test]
    fn parse_line_with_valid_name_prefix() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("server:restart");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "restart");
        assert_eq!(parsed.target_name, "server");
    }

    #[test]
    fn parse_line_with_valid_index_prefix() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("1:hello");
        assert_eq!(parsed.target, 1);
        assert_eq!(parsed.payload, "hello");
        assert_eq!(parsed.target_name, "worker");
    }

    #[test]
    fn parse_line_no_colon_goes_to_default() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("hello world");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "hello world");
    }

    #[test]
    fn parse_line_unresolved_prefix_goes_to_default() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("http://localhost:3000");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "http://localhost:3000");
    }

    #[test]
    fn parse_line_multiple_colons_splits_on_first() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        let parsed = router.parse_line("server:key:value");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "key:value");
    }

    #[test]
    fn parse_line_out_of_range_index_goes_to_default() {
        let router = make_router(vec!["server", "worker"], 2, 1);
        let parsed = router.parse_line("99:hello");
        assert_eq!(parsed.target, 1);
        assert_eq!(parsed.payload, "99:hello");
    }

    #[test]
    fn parse_line_empty_line_goes_to_default() {
        let router = make_router(vec!["server"], 1, 0);
        let parsed = router.parse_line("");
        assert_eq!(parsed.target, 0);
        assert_eq!(parsed.payload, "");
    }

    #[test]
    fn parse_line_with_non_default_target() {
        let router = make_router(vec!["server", "worker"], 2, 1);
        let parsed = router.parse_line("hello");
        assert_eq!(parsed.target, 1);
        assert_eq!(parsed.payload, "hello");
    }

    #[test]
    fn display_name_with_named_process() {
        let router = make_router(vec!["server", "worker"], 2, 0);
        assert_eq!(router.display_name(0), "server");
        assert_eq!(router.display_name(1), "worker");
    }

    #[test]
    fn display_name_falls_back_to_index() {
        let router = make_router(vec![], 2, 0);
        assert_eq!(router.display_name(0), "0");
        assert_eq!(router.display_name(1), "1");
    }

    #[tokio::test]
    async fn register_and_deregister() {
        let router = make_router(vec!["server"], 1, 0);
        // Initially empty
        assert!(router.handles.lock().await.is_empty());

        // We can't easily create a real ChildStdin in a test, but we can
        // verify the map operations work by checking the handles map size
        // after deregister of a non-existent key (no panic).
        router.deregister(0).await;
        assert!(router.handles.lock().await.is_empty());
    }
}
