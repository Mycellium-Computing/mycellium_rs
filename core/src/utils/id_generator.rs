use core::sync::atomic::{AtomicU32, Ordering};

/// A thread-safe atomic ID generator that produces unique sequential IDs.
/// Uses `core::sync::atomic::AtomicU32` to ensure thread safety without external dependencies.
pub struct AtomicIdGenerator {
    counter: AtomicU32,
}

impl AtomicIdGenerator {
    /// Creates a new `AtomicIdGenerator` starting from 0.
    pub const fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
        }
    }

    /// Creates a new `AtomicIdGenerator` starting from a specific value.
    pub const fn with_start(start: u32) -> Self {
        Self {
            counter: AtomicU32::new(start),
        }
    }

    /// Generates the next unique ID.
    /// This method is thread-safe and will always return a unique value
    /// across all threads (until overflow).
    pub fn next_id(&self) -> u32 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Returns the current value without incrementing.
    pub fn current(&self) -> u32 {
        self.counter.load(Ordering::Relaxed)
    }

    /// Resets the counter to 0.
    pub fn reset(&self) {
        self.counter.store(0, Ordering::Relaxed);
    }
}

impl Default for AtomicIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Global static ID generator for request IDs.
/// This provides a single source of unique IDs across the entire application.
static GLOBAL_REQUEST_ID_GENERATOR: AtomicIdGenerator = AtomicIdGenerator::new();

/// Generates the next unique request ID using the global generator.
/// This is the recommended way to generate request IDs for `ProviderExchange` messages.
pub fn next_request_id() -> u32 {
    GLOBAL_REQUEST_ID_GENERATOR.next_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_ids() {
        let generator = AtomicIdGenerator::new();
        assert_eq!(generator.next_id(), 0);
        assert_eq!(generator.next_id(), 1);
        assert_eq!(generator.next_id(), 2);
    }

    #[test]
    fn test_with_start() {
        let generator = AtomicIdGenerator::with_start(100);
        assert_eq!(generator.next_id(), 100);
        assert_eq!(generator.next_id(), 101);
    }

    #[test]
    fn test_current() {
        let generator = AtomicIdGenerator::new();
        assert_eq!(generator.current(), 0);
        generator.next_id();
        assert_eq!(generator.current(), 1);
    }

    #[test]
    fn test_reset() {
        let generator = AtomicIdGenerator::new();
        generator.next_id();
        generator.next_id();
        generator.reset();
        assert_eq!(generator.current(), 0);
    }
}
