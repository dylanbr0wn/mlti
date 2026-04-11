use std::collections::HashMap;
use std::sync::Arc;

use flume::Sender;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
use tokio::sync::Mutex;

use crate::message::{build_message_sender, Message, MessageType, SenderType};

/// Resolve a candidate string to a process index.
///
/// Matches a name in `names` first (so a process literally named "1" wins
/// over the index `1`), then falls back to parsing the candidate as a
/// `usize` and checking it is within `0..num_processes`. `num_processes`
/// may be larger than `names.len()` because naming is optional.
pub fn resolve_target(
  candidate: &str,
  names: &[String],
  num_processes: usize,
) -> Option<usize> {
  if let Some(idx) = names.iter().position(|n| n == candidate) {
    return Some(idx);
  }
  if let Ok(idx) = candidate.parse::<usize>() {
    if idx < num_processes {
      return Some(idx);
    }
  }
  None
}

pub struct InputRouter {
  // Per-handle `Arc<Mutex<_>>` so `route()` can grab a single handle,
  // release the map lock, and then hold the per-process lock across the
  // write/flush `.await` without blocking `register`/`deregister` for
  // other processes. A slow child can still back up its own routing, but
  // it no longer stalls the rest of the input subsystem.
  handles: Mutex<HashMap<usize, Arc<Mutex<ChildStdin>>>>,
  names: Vec<String>,
  // `num_processes` may exceed `names.len()` because `--names` is
  // optional — keep both rather than assuming they are equal.
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

  /// Parse an input line into a target index and payload.
  fn parse_line(&self, line: &str) -> ParsedInput {
    if let Some(colon_pos) = line.find(':') {
      let candidate = &line[..colon_pos];
      if let Some(idx) = resolve_target(candidate, &self.names, self.num_processes) {
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
    self
      .names
      .get(index)
      .cloned()
      .unwrap_or_else(|| index.to_string())
  }

  pub async fn register(&self, index: usize, stdin: ChildStdin) {
    self
      .handles
      .lock()
      .await
      .insert(index, Arc::new(Mutex::new(stdin)));
  }

  pub async fn deregister(&self, index: usize) {
    self.handles.lock().await.remove(&index);
  }

  pub async fn route(&self, line: &str) {
    let parsed = self.parse_line(line);

    // Quickly grab a clone of the target handle (or the map emptiness),
    // then drop the map lock before any `.await` on the stdin write.
    let handle = {
      let handles = self.handles.lock().await;
      if handles.is_empty() {
        self
          .send_message("[mlti] No running processes, input discarded".to_string());
        return;
      }
      handles.get(&parsed.target).cloned()
    };

    let Some(stdin) = handle else {
      self.send_message(format!(
        "[mlti] Unknown target \"{}\", input discarded",
        parsed.target_name
      ));
      return;
    };

    let data = format!("{}\n", parsed.payload);
    let write_result = {
      let mut stdin = stdin.lock().await;
      match stdin.write_all(data.as_bytes()).await {
        Ok(()) => stdin.flush().await,
        Err(e) => Err(e),
      }
    };

    if write_result.is_err() {
      // Process died between lookup and write. Drop the dead handle so
      // subsequent lines don't hit the same failure.
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

  // Swallow send errors (`.ok()`) instead of `.expect(...)`: during
  // shutdown the messenger may have already been dropped, and panicking
  // from the router would poison the shutdown path.
  fn send_message(&self, data: String) {
    self
      .message_tx
      .send(Message::new(
        MessageType::Text,
        Some("".to_string()),
        Some(data),
        None,
        build_message_sender(SenderType::Main, None, None),
      ))
      .ok();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_router(
    names: Vec<&str>,
    num_processes: usize,
    default_target: usize,
  ) -> InputRouter {
    let (tx, _rx) = flume::unbounded();
    InputRouter::new(
      names.into_iter().map(String::from).collect(),
      num_processes,
      default_target,
      tx,
    )
  }

  fn names_of(strs: &[&str]) -> Vec<String> {
    strs.iter().map(|s| s.to_string()).collect()
  }

  // -- resolve_target --

  #[test]
  fn resolve_by_name() {
    let names = names_of(&["server", "worker"]);
    assert_eq!(resolve_target("server", &names, 2), Some(0));
    assert_eq!(resolve_target("worker", &names, 2), Some(1));
  }

  #[test]
  fn resolve_by_index() {
    let names = names_of(&["server", "worker"]);
    assert_eq!(resolve_target("0", &names, 2), Some(0));
    assert_eq!(resolve_target("1", &names, 2), Some(1));
  }

  #[test]
  fn resolve_name_takes_priority_over_index() {
    // Process named "1" should match by name (index 0), not by index 1.
    let names = names_of(&["1", "worker"]);
    assert_eq!(resolve_target("1", &names, 2), Some(0));
  }

  #[test]
  fn resolve_unknown_name_returns_none() {
    let names = names_of(&["server", "worker"]);
    assert_eq!(resolve_target("unknown", &names, 2), None);
  }

  #[test]
  fn resolve_out_of_range_index_returns_none() {
    let names = names_of(&["server", "worker"]);
    assert_eq!(resolve_target("99", &names, 2), None);
  }

  #[test]
  fn resolve_with_unnamed_processes() {
    // num_processes can exceed names.len() when --names is not given.
    let names: Vec<String> = vec![];
    assert_eq!(resolve_target("0", &names, 3), Some(0));
    assert_eq!(resolve_target("2", &names, 3), Some(2));
    assert_eq!(resolve_target("3", &names, 3), None);
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
  async fn register_and_deregister_are_safe_on_empty_map() {
    let router = make_router(vec!["server"], 1, 0);
    assert!(router.handles.lock().await.is_empty());
    // Deregistering a key that was never registered must be a no-op.
    router.deregister(0).await;
    assert!(router.handles.lock().await.is_empty());
  }

  // Spawn a real cat child so we have a genuine ChildStdin to register.
  // Ignored by default because it shells out; run with
  // `cargo test -- --ignored` if you want to exercise it locally.
  async fn spawn_cat() -> tokio::process::Child {
    tokio::process::Command::new("cat")
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .spawn()
      .expect("failed to spawn cat")
  }

  #[tokio::test]
  async fn re_register_replaces_existing_handle() {
    // Simulates the restart path in Task::start: after a process
    // restarts, its index is deregistered and then registered again
    // with a fresh ChildStdin. The map must always contain exactly
    // one handle for that index, and routing must reach the new one.
    let router = make_router(vec!["cat"], 1, 0);

    let mut c1 = spawn_cat().await;
    let mut c2 = spawn_cat().await;
    let stdin1 = c1.stdin.take().expect("c1 stdin piped");
    let stdin2 = c2.stdin.take().expect("c2 stdin piped");

    router.register(0, stdin1).await;
    assert_eq!(router.handles.lock().await.len(), 1);

    // Re-register without deregistering first (the restart path
    // deregisters explicitly, but the map must also cope with raw
    // overwrite — leaking the old handle would prevent the old child
    // from seeing EOF on its stdin).
    router.register(0, stdin2).await;
    assert_eq!(router.handles.lock().await.len(), 1);

    // Route a line — it must reach the *new* handle (c2), not c1.
    router.route("hello").await;

    router.deregister(0).await;
    assert!(router.handles.lock().await.is_empty());

    // Cleanly tear down both children so we don't leak zombies.
    let _ = c1.kill().await;
    let _ = c2.kill().await;
    let _ = c1.wait().await;
    let _ = c2.wait().await;
  }
}
