use tracing::Span;

/// Container for holding span context
#[derive(Debug, Clone)]
pub struct Message<D> {
    pub data: D,
    pub span: Span,
}

impl<D> Message<D> {
    /// Create a new message with the given data, using the current span
    pub fn new(data: D) -> Self {
        Message {
            data,
            span: tracing::Span::current(),
        }
    }

    /// Create a new message with the given data and span
    pub fn new_with_span(data: D, span: Span) -> Self {
        Message { data, span }
    }

    /// Destructures the message into its data and span, consuming the message
    pub fn destructure(self) -> (D, Span) {
        (self.data, self.span)
    }
}
