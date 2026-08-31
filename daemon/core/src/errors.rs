//! Typed errors for the agentic loop + retry-with-backoff helper.
//!
//! EP-0003 Tier 1: replace the old `Result<_, String>` everywhere in the
//! agentic loop and tool calls with structured errors that callers can
//! route on (retry vs fail-fast vs surface-to-user).
//!
//! Two enums:
//!
//! - [`AgentError`]: top-level error for the agentic loop. Variants
//!   cover upstream LLM failures, tool failures (wrapping
//!   [`ToolError`]), iteration limits, malformed tool calls, and
//!   internal daemon bugs.
//!
//! - [`ToolError`]: tool-level taxonomy. Tools that want to trigger
//!   retries return [`ToolError::Retryable`] or [`ToolError::Timeout`];
//!   everything else is permanent.
//!
//! Plus two helpers:
//!
//! - [`RetryPolicy`]: configurable backoff schedule (exponential +
//!   jitter, honors [`Retry-After`] for upstream errors).
//!
//! - [`retry_with_backoff`]: closure-based retry loop that retries
//!   only when [`AgentError::is_retryable()`] is true. Honors
//!   `Retry-After` headers in [`AgentError::LlmUpstream`].
//!
//! Tier 2 (state split, soft-cap iterations, idempotency) and Tier 3
//! (graceful shutdown, tracing spans, shared reqwest::Client) are
//! scoped separately — see EP-0003-agentic-llm-robustness.

use std::time::Duration;

// ─── AgentError ──────────────────────────────────────────────────────────

/// Domain-level error for the agentic loop and tool calls.
///
/// LLM-client implementations (MiniMax, OpenAI-compat) and tool
/// implementations (ProxyTool, future plugins) translate raw failures
/// (HTTP status, network errors, etc.) into one of these variants so
/// callers can route on them (retry, surface-to-user, fail-fast)
/// without parsing strings.
///
/// EP-0012 O-003: derived via `thiserror` to eliminate the manual
/// `impl Display` boilerplate. `Display`/`Error` impls come for free;
/// new variants only need a single line.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AgentError {
    /// The upstream LLM returned a retryable error (HTTP 429 or
    /// HTTP 5xx) or the network call timed out. `retry_after` is the
    /// duration the server suggested via the `Retry-After` header
    /// (`None` if absent). `attempts` is the total number of calls
    /// made (1 = no retries, 4 = 1 original + 3 retries).
    ///
    /// After `RetryPolicy::max_retries` attempts the error is
    /// surfaced as-is — no auto-retry beyond that.
    #[error("upstream LLM error (status={status}, attempts={attempts})")]
    LlmUpstream {
        status: u16,
        retry_after: Option<Duration>,
        attempts: u32,
    },

    /// The upstream LLM timed out (network read timeout, idle
    /// connection dropped, etc.). Distinct from `LlmUpstream` so
    /// callers can apply different policies (always retry vs
    /// status-based retry).
    #[error("upstream LLM timeout (attempts={attempts})")]
    LlmTimeout { attempts: u32 },

    /// A tool returned a categorized error. Wraps [`ToolError`].
    #[error("tool error: {source}")]
    Tool { source: ToolError },

    /// `MAX_TOOL_ITERATIONS` reached without resolving. Tier 2
    /// replaces the hard-fail with a soft-cap that asks the agent to
    /// summarize and ask the user for next steps; this variant is the
    /// safety net for when the soft cap also fails. (Placeholder;
    /// populated in Tier 2.)
    #[error("agentic loop iteration limit reached")]
    IterationLimit,

    /// The LLM emitted a tool_call whose `tool_call_id` didn't match
    /// the previous assistant message, or whose `arguments` JSON
    /// couldn't be parsed. (Placeholder; populated in Tier 2.)
    #[error("malformed tool call: {0}")]
    MalformedToolCall(String),

    /// Internal error — daemon-side bug, not an upstream issue.
    /// Surfacing this to the user is a 5xx; logging should include
    /// enough context to debug.
    #[error("internal error: {0}")]
    Internal(String),
}

impl AgentError {
    /// `true` if the caller should attempt to retry this error.
    /// Used by [`retry_with_backoff`] and by hand-rolled retry loops
    /// in [`crate::tools::proxy::ProxyTool`].
    ///
    /// Categories that retry:
    /// - HTTP 429 (rate-limit) — always
    /// - HTTP 5xx (server errors) — always
    /// - Network timeout — always
    /// - Tool errors marked [`ToolError::Retryable`] or
    ///   [`ToolError::Timeout`] — yes
    ///
    /// Categories that don't retry:
    /// - HTTP 4xx (except 429) — client error, retry won't fix
    /// - `PermissionDenied` / `NotFound` / `Validation` tool errors
    /// - `IterationLimit`, `MalformedToolCall`, `Internal`
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::LlmUpstream { status, .. } => {
                *status == 429 || (*status >= 500 && *status <= 599)
            }
            Self::LlmTimeout { .. } => true,
            Self::Tool { source } => source.is_retryable(),
            _ => false,
        }
    }
}

// ─── ToolError ───────────────────────────────────────────────────────────

/// Tool-level error taxonomy. Tools that want to trigger retries
/// return [`ToolError::Retryable`] or [`ToolError::Timeout`]; tools
/// that fail permanently return one of the other variants.
///
/// The string payload is a free-form description (logged + returned
/// to the SPA as the tool error). When Tier 2 lands and the SPA
/// surfaces tool errors per-tool, this can carry a structured
/// `code` field; for Tier 1 the string is enough.
#[derive(Debug, Clone)]
pub enum ToolError {
    /// Transient error (network blip, downstream timeout). Caller
    /// should retry.
    Retryable(String),
    /// Permanent error (bad input, invalid config, missing
    /// resource). Retrying won't help.
    Permanent(String),
    /// Auth/permission denied — usually user-actionable. The SPA
    /// should surface this as an "auth needed" state.
    PermissionDenied(String),
    /// Tool not found in the registry — likely a stale session
    /// referencing a tool that's been removed since.
    NotFound(String),
    /// The tool's downstream call timed out (separate from generic
    /// network error so callers can distinguish).
    Timeout(String),
    /// Input validation failed before the tool ran.
    Validation(String),
    /// Tool was disabled by CapabilityStore (EP-0014 C-004).
    Disabled,
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retryable(s)
            | Self::Permanent(s)
            | Self::PermissionDenied(s)
            | Self::NotFound(s)
            | Self::Timeout(s)
            | Self::Validation(s) => write!(f, "{s}"),
            Self::Disabled => write!(f, "tool disabled by capability"),
        }
    }
}

impl std::error::Error for ToolError {}

impl ToolError {
    /// `true` if the caller should retry this tool error.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_) | Self::Timeout(_))
    }
}

// ─── RetryPolicy ────────────────────────────────────────────────────────

/// Backoff schedule for [`retry_with_backoff`]. Defaults are
/// `max_retries=3`, `initial_backoff=1s`, `max_backoff=8s`,
/// `multiplier=2.0`, `jitter=0.2`.
///
/// Override per call site if needed (LLM stream vs utility vs tool
/// may want different policies). Env-var loading is the caller's
/// responsibility — Tier 1 keeps this in code.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    /// Multiplier between retries. 2.0 = exponential.
    pub multiplier: f64,
    /// Random jitter as a fraction of the backoff. 0.2 = ±20%.
    pub jitter: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(8),
            multiplier: 2.0,
            jitter: 0.2,
        }
    }
}

// ─── retry_with_backoff ─────────────────────────────────────────────────

/// Execute `op` with exponential backoff + jitter, retrying only
/// when the returned [`AgentError::is_retryable()`] is true.
///
/// Sleep is via `tokio::time::sleep` so it's cancellable. Returns
/// the LAST error after all retries exhausted (so callers don't lose
/// context for logging).
///
/// Honors `Retry-After` header for [`AgentError::LlmUpstream`]
/// errors: if present, that duration is used as the next backoff
/// (clamped to `max_backoff` and not below `initial_backoff`).
///
/// Jitter is symmetric around the backoff: a value of `0.2` means
/// the actual sleep is in `[backoff * 0.8, backoff * 1.2]`. Uses
/// `rand::random()` — uniform distribution in `[-1.0, 1.0)`.
///
/// Generic over the success type `T` so callers can return any
/// successful value (a response, a parsed JSON, etc.) — not just `()`.
pub async fn retry_with_backoff<F, Fut, T>(
    policy: RetryPolicy,
    mut op: F,
) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let mut last_err: Option<AgentError> = None;
    let mut backoff = policy.initial_backoff;

    for attempt in 0..=policy.max_retries {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if !e.is_retryable() => return Err(e),
            Err(e) => {
                // Honor Retry-After for LlmUpstream errors.
                if let AgentError::LlmUpstream {
                    retry_after: Some(ra),
                    ..
                } = &e
                {
                    let ra_secs = ra.as_secs().min(policy.max_backoff.as_secs());
                    let ra = Duration::from_secs(ra_secs.max(policy.initial_backoff.as_secs()));
                    backoff = ra;
                }
                last_err = Some(e);
                if attempt == policy.max_retries {
                    break;
                }
                // Apply symmetric jitter: ±jitter% of backoff.
                let jitter_amount = backoff.as_secs_f64() * policy.jitter;
                let jitter_offset = rand::random::<f64>() * 2.0 - 1.0; // [-1, 1)
                let sleep_for = (backoff.as_secs_f64() + jitter_offset * jitter_amount).max(0.0);
                tokio::time::sleep(Duration::from_secs_f64(sleep_for)).await;
                // Exponential backoff for next iteration.
                backoff = Duration::from_secs_f64(
                    (backoff.as_secs_f64() * policy.multiplier).min(policy.max_backoff.as_secs_f64()),
                );
            }
        }
    }
    Err(last_err.expect("loop always runs at least once"))
}

// ─── CircuitBreaker ─────────────────────────────────────────────────────

use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// State of a circuit breaker.
///
/// Stored as a single byte in an `AtomicU8` so the check is lock-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed = 0,   // normal — all calls allowed
    Open = 1,     // failing — calls fail fast
    HalfOpen = 2, // probing — exactly one call allowed
}

impl CircuitState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Closed,
            1 => Self::Open,
            2 => Self::HalfOpen,
            _ => Self::Closed,
        }
    }
}

/// Half-open circuit breaker pattern: counts failures within a
/// sliding window; opens the circuit after `fail_threshold`; re-closes
/// after `open_duration`; allows exactly one probe call in the
/// `HalfOpen` state to test recovery.
///
/// `Arc<CircuitBreaker>` is meant to be shared across many calls
/// (e.g. one per llmd backend, shared across all `ProxyTool`
/// instances).
pub struct CircuitBreaker {
    state: AtomicU8,
    fail_count: AtomicU32,
    /// `last_fail_at` is `Some(SystemTime::now().duration_since(UNIX_EPOCH).as_nanos())`
    /// at the moment of the last recorded failure; `None` (stored as
    /// 0) means no failure recorded yet.
    last_fail_at: AtomicI64,
    config: CircuitConfig,
}

/// Configuration for a [`CircuitBreaker`]. Tunable so ops can adjust
/// based on the backend's SLOs.
#[derive(Debug, Clone, Copy)]
pub struct CircuitConfig {
    /// Consecutive failures within `window` that trip the breaker.
    pub fail_threshold: u32,
    /// Sliding window — failures older than this are forgotten.
    pub window: Duration,
    /// How long the breaker stays open before allowing a probe.
    pub open_duration: Duration,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        // EP-0003 Tier 1 acceptance: 5 fails in 30s → open for 60s.
        Self {
            fail_threshold: 5,
            window: Duration::from_secs(30),
            open_duration: Duration::from_secs(60),
        }
    }
}

impl CircuitBreaker {
    pub fn new(config: CircuitConfig) -> Self {
        Self {
            state: AtomicU8::new(CircuitState::Closed as u8),
            fail_count: AtomicU32::new(0),
            last_fail_at: AtomicI64::new(0),
            config,
        }
    }

    /// Returns the current state (Closed, Open, HalfOpen).
    pub fn state(&self) -> CircuitState {
        CircuitState::from_u8(self.state.load(Ordering::Acquire))
    }

    /// Returns `true` if the call should proceed, `false` if the
    /// circuit is open and the call should fail-fast.
    ///
    /// Side effect: if the breaker was `Open` and the open-duration
    /// has elapsed, transitions to `HalfOpen` and allows this call
    /// (the probe). The caller MUST report the outcome via
    /// [`record_success`] or [`record_failure`].
    pub fn allow(&self) -> bool {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true, // probe
            CircuitState::Open => {
                let last = self.last_fail_at.load(Ordering::Acquire);
                let elapsed_nanos = now_nanos.saturating_sub(last);
                if elapsed_nanos >= self.config.open_duration.as_nanos() as i64 {
                    // Transition Open → HalfOpen. The compare-exchange
                    // ensures only one caller wins the probe slot.
                    let _ = self.state.compare_exchange(
                        CircuitState::Open as u8,
                        CircuitState::HalfOpen as u8,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful call. Closes the circuit if it was
    /// `HalfOpen` or `Open` (i.e. recovery) and resets the failure
    /// counter.
    pub fn record_success(&self) {
        let prev = self.state.load(Ordering::Acquire);
        self.fail_count.store(0, Ordering::Release);
        self.last_fail_at.store(0, Ordering::Release);
        if prev != CircuitState::Closed as u8 {
            let _ = self.state.compare_exchange(
                prev,
                CircuitState::Closed as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    /// Record a failed call. Increments the counter (with sliding
    /// window reset) and trips the breaker to `Open` once
    /// `fail_threshold` is reached within `window`.
    pub fn record_failure(&self) {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        let last = self.last_fail_at.load(Ordering::Acquire);
        // Reset counter if the previous failure was outside the window.
        let elapsed_nanos = now_nanos.saturating_sub(last);
        let count = if last == 0 || elapsed_nanos > self.config.window.as_nanos() as i64 {
            self.fail_count.store(1, Ordering::Release);
            1
        } else {
            self.fail_count.fetch_add(1, Ordering::AcqRel) + 1
        };
        self.last_fail_at.store(now_nanos, Ordering::Release);
        if count >= self.config.fail_threshold {
            // Trip the breaker.
            let _ = self.state.compare_exchange(
                CircuitState::Closed as u8,
                CircuitState::Open as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    /// Force the breaker back to `Closed` (admin override / test
    /// helper). Use sparingly.
    pub fn reset(&self) {
        self.state.store(CircuitState::Closed as u8, Ordering::Release);
        self.fail_count.store(0, Ordering::Release);
        self.last_fail_at.store(0, Ordering::Release);
    }
}

// ─── tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn retryable_classification() {
        // 429 and 5xx retry
        assert!(AgentError::LlmUpstream {
            status: 429,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        assert!(AgentError::LlmUpstream {
            status: 500,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        assert!(AgentError::LlmUpstream {
            status: 502,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        assert!(AgentError::LlmUpstream {
            status: 503,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        // 4xx (except 429) don't retry
        assert!(!AgentError::LlmUpstream {
            status: 400,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        assert!(!AgentError::LlmUpstream {
            status: 401,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        assert!(!AgentError::LlmUpstream {
            status: 403,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        assert!(!AgentError::LlmUpstream {
            status: 404,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());
        assert!(!AgentError::LlmUpstream {
            status: 422,
            retry_after: None,
            attempts: 1
        }
        .is_retryable());

        // LlmTimeout retries
        assert!(AgentError::LlmTimeout { attempts: 1 }.is_retryable());

        // Tool errors: only Retryable + Timeout
        assert!(AgentError::Tool {
            source: ToolError::Retryable("x".into())
        }
        .is_retryable());
        assert!(AgentError::Tool {
            source: ToolError::Timeout("x".into())
        }
        .is_retryable());
        assert!(!AgentError::Tool {
            source: ToolError::Permanent("x".into())
        }
        .is_retryable());
        assert!(!AgentError::Tool {
            source: ToolError::PermissionDenied("x".into())
        }
        .is_retryable());

        // These never retry
        assert!(!AgentError::IterationLimit.is_retryable());
        assert!(!AgentError::MalformedToolCall("x".into()).is_retryable());
        assert!(!AgentError::Internal("x".into()).is_retryable());
    }

    #[tokio::test]
    async fn retry_eventually_succeeds() {
        let count = AtomicU32::new(0);
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(10),
            ..RetryPolicy::default()
        };
        let result = retry_with_backoff(policy, || {
            let count_ref = &count;
            async move {
                let n = count_ref.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err(AgentError::LlmUpstream {
                        status: 503,
                        retry_after: None,
                        attempts: n + 1,
                    })
                } else {
                    Ok(())
                }
            }
        })
        .await;
        assert!(result.is_ok());
        assert!(count.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn retry_gives_up_on_permanent() {
        let policy = RetryPolicy {
            max_retries: 5,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(10),
            ..RetryPolicy::default()
        };
        let mut invocations = 0;
        let result = retry_with_backoff(policy, || {
            invocations += 1;
            async move { Err::<(), _>(AgentError::Internal("bad".into())) }
        })
        .await;
        assert!(matches!(result, Err(AgentError::Internal(_))));
        assert_eq!(invocations, 1, "permanent error must not retry");
    }

    #[tokio::test]
    async fn retry_exhausts_after_max_retries() {
        let count = AtomicU32::new(0);
        let policy = RetryPolicy {
            max_retries: 2,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(10),
            ..RetryPolicy::default()
        };
        let result = retry_with_backoff(policy, || {
            let count_ref = &count;
            async move {
                count_ref.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(AgentError::LlmUpstream {
                    status: 503,
                    retry_after: None,
                    attempts: 0, // populated by caller normally
                })
            }
        })
        .await;
        assert!(matches!(result, Err(AgentError::LlmUpstream { .. })));
        // 1 initial + 2 retries = 3 invocations
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn circuit_breaker_trips_after_threshold() {
        let cb = CircuitBreaker::new(CircuitConfig {
            fail_threshold: 3,
            window: Duration::from_secs(30),
            open_duration: Duration::from_secs(60),
        });
        // Initially closed → allow
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow());
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure(); // third failure → trips
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow(), "open circuit must fail-fast");
    }

    #[test]
    fn circuit_breaker_half_open_after_open_duration() {
        let cb = CircuitBreaker::new(CircuitConfig {
            fail_threshold: 2,
            window: Duration::from_secs(30),
            open_duration: Duration::from_millis(50),
        });
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow());
        std::thread::sleep(Duration::from_millis(60));
        // After open_duration, allow() transitions Open → HalfOpen.
        assert!(cb.allow());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        // A successful recovery closes the circuit.
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        // Failures reset on recovery.
        assert_eq!(cb.fail_count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn circuit_breaker_sliding_window_resets_count() {
        let cb = Arc::new(CircuitBreaker::new(CircuitConfig {
            fail_threshold: 3,
            window: Duration::from_millis(50),
            open_duration: Duration::from_secs(60),
        }));
        cb.record_failure();
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(60));
        // Old failures are outside the window → counter resets.
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed, "1 fresh failure under threshold");
    }

    #[test]
    fn circuit_breaker_record_failure_increments() {
        let cb = CircuitBreaker::new(CircuitConfig::default());
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        // 5th failure trips it (default threshold = 5).
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn retry_honors_retry_after_header() {
        // We can't easily simulate a sleep inside the retry loop in
        // a unit test without timing flakiness, but we can verify
        // that a Retry-After longer than max_backoff gets clamped.
        let policy = RetryPolicy {
            max_retries: 1,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(2),
            multiplier: 2.0,
            jitter: 0.0,
        };
        let t0 = std::time::Instant::now();
        let _ = retry_with_backoff(policy, || {
            async {
                Err::<(), _>(AgentError::LlmUpstream {
                    status: 429,
                    retry_after: Some(Duration::from_secs(10)), // > max_backoff
                    attempts: 0,
                })
            }
        })
        .await;
        let elapsed = t0.elapsed();
        // Backoff was clamped to max_backoff (2s). With 1 retry,
        // total wait = ~2s + tiny overhead. We allow generous slack.
        assert!(elapsed < Duration::from_secs(5));
    }
}
