pub trait Event {
    fn event_type(&self) -> &'static str;
}

pub trait EventBus {
    fn publish(&self, event: Box<dyn Event>);
}
