use std::{
    slice::Iter,
    time::{Duration, Instant},
};

/// [EventContext] provides context around who sent a particular event, and
/// timing information around it.
#[derive(Debug, Clone)]
pub struct EventContext {
    /// Performance counter with spans for measuring event latency
    metrics: PerformanceCounter,
}

impl EventContext {
    /// Create new context for an event
    pub fn new() -> Self {
        Self {
            metrics: PerformanceCounter::new(),
        }
    }

    /// Return performance metrics counter for the event
    pub fn metrics(&self) -> &PerformanceCounter {
        &self.metrics
    }

    /// Return performance metrics counter for the event
    pub fn metrics_mut(&mut self) -> &mut PerformanceCounter {
        &mut self.metrics
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}

/// [PerformanceCounter] keeps an array of spans to keep track of how long
/// different points of the input pipeline took.
#[derive(Debug, Clone)]
pub struct PerformanceCounter {
    spans: Vec<Span>,
}

impl PerformanceCounter {
    pub fn new() -> Self {
        Self::with_capacity(8)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            spans: Vec::with_capacity(capacity),
        }
    }

    /// Create a span with the given identifier
    pub fn create_span(&mut self, id: &'static str) -> &mut Span {
        let span = Span::new(id);
        self.spans.push(span);
        self.spans.last_mut().unwrap()
    }

    /// Create a span that is a child of the given span `parent_id` with the
    /// given identifier.
    pub fn create_child_span(&mut self, parent_id: &'static str, id: &'static str) -> &mut Span {
        let span = self.create_span(id);
        span.parent_id = Some(parent_id);
        span
    }

    /// Return the span with the given span id
    #[allow(dead_code)]
    pub fn get(&self, id: &'static str) -> Option<&Span> {
        self.spans.iter().find(|span| span.id() == id)
    }

    /// Returns the span with the given span id
    pub fn get_mut(&mut self, id: &'static str) -> Option<&mut Span> {
        self.spans.iter_mut().find(|span| span.id() == id)
    }

    pub fn iter(&self) -> Iter<'_, Span> {
        self.spans.iter()
    }
}

impl Default for PerformanceCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// A [Span] keeps track of a start time and end time in order to calculate how
/// long an operation took.
#[derive(Debug, Clone, Copy)]
pub struct Span {
    id: &'static str,
    parent_id: Option<&'static str>,
    start_time: Option<Instant>,
    duration: Option<Duration>,
}

impl Span {
    /// Create a new unstarted span. Requires calling `start()` in order to
    /// record the start time.
    fn new(id: &'static str) -> Self {
        Self {
            id,
            parent_id: None,
            start_time: None,
            duration: None,
        }
    }

    /// Identifier of the span
    pub fn id(&self) -> &str {
        self.id
    }

    /// Identifier of the parent span
    pub fn parent_id(&self) -> Option<&str> {
        self.parent_id
    }

    /// Start the span
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// Finish the span and calculate the elapsed duration since start
    pub fn finish(&mut self) {
        // Don't do anything if this span has already finished
        if self.duration.is_some() {
            return;
        }
        let Some(start_time) = self.start_time.as_ref() else {
            return;
        };
        self.duration = Some(start_time.elapsed());
    }

    /// Return the elapsed time since the span started and finished
    pub fn elapsed(&self) -> Option<Duration> {
        self.duration
    }
}

/// Serialized version of a [Span], in the form of (parent_id, id, elapsed_micro_sec)
pub type SerializedSpan = (String, String, u64);

impl From<&Span> for SerializedSpan {
    fn from(value: &Span) -> Self {
        let parent_id = value.parent_id().unwrap_or_default().to_string();
        let id = value.id().to_string();
        let elapsed = value.elapsed().unwrap_or_default().as_micros() as u64;
        (parent_id, id, elapsed)
    }
}
