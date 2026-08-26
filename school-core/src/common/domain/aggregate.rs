use super::event::DomainEvent;
use uuid::Uuid;

/// Base trait for all Aggregate Roots in the system.
/// This defines the minimum contract that every aggregate must satisfy.
pub trait AggregateRoot {
    /// Returns the unique identifier of the aggregate.
    fn id(&self) -> Uuid;

    /// Returns the current version of the aggregate for optimistic concurrency.
    fn version(&self) -> i32;

    /// Extracts all pending domain events from the aggregate.
    /// This should clear the internal events list.
    fn take_events(&mut self) -> Vec<Box<dyn DomainEvent>>;
}
