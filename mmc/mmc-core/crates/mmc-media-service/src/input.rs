//! Input event handling module for touch and keyboard events

use mmc_protocol::{TouchEvent, KeyEvent, TouchType, KeyEventType};
use tracing::debug;
use crate::error::Result;

/// Input event types
#[derive(Debug, Clone)]
pub enum InputEvent {
    Touch(TouchEvent),
    Key(KeyEvent),
}

/// Input handler trait
///
/// Implement this trait to handle incoming input events.
pub trait InputHandler: Send + Sync {
    fn handle_touch(&mut self, event: &TouchEvent) -> Result<()>;
    fn handle_key(&mut self, event: &KeyEvent) -> Result<()>;
}

/// Default input handler that logs events
#[derive(Debug, Default)]
pub struct DefaultInputHandler {
    touch_count: u64,
    key_count: u64,
}

impl DefaultInputHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn touch_count(&self) -> u64 {
        self.touch_count
    }

    pub fn key_count(&self) -> u64 {
        self.key_count
    }
}

impl InputHandler for DefaultInputHandler {
    fn handle_touch(&mut self, event: &TouchEvent) -> Result<()> {
        self.touch_count += 1;
        debug!("Touch event: {:?} at ({:.2}, {:.2}) pressure={:.2}",
            event.touch_type,
            event.x,
            event.y,
            event.pressure
        );
        Ok(())
    }

    fn handle_key(&mut self, event: &KeyEvent) -> Result<()> {
        self.key_count += 1;
        debug!("Key event: {:?} code={}, text={:?}",
            event.key_type,
            event.key_code,
            event.text
        );
        Ok(())
    }
}

/// Input dispatcher
///
/// Dispatches incoming input events to registered handlers.
#[derive(Debug)]
pub struct InputDispatcher {
    event_count: u64,
    last_event_time: Option<std::time::Instant>,
}

impl InputDispatcher {
    pub fn new() -> Self {
        Self {
            event_count: 0,
            last_event_time: None,
        }
    }

    pub fn dispatch_to_handler(&mut self, event: &InputEvent, handler: &mut dyn InputHandler) -> Result<()> {
        self.event_count += 1;
        self.last_event_time = Some(std::time::Instant::now());

        match event {
            InputEvent::Touch(touch) => handler.handle_touch(touch),
            InputEvent::Key(key) => handler.handle_key(key),
        }
    }

    pub fn dispatch(&mut self, event: &InputEvent) -> Result<()> {
        self.event_count += 1;
        self.last_event_time = Some(std::time::Instant::now());

        match event {
            InputEvent::Touch(touch) => {
                debug!("Dispatched touch event: seq={}", touch.sequence_id);
                Ok(())
            }
            InputEvent::Key(key) => {
                debug!("Dispatched key event: seq={}", key.sequence_id);
                Ok(())
            }
        }
    }

    pub fn event_count(&self) -> u64 {
        self.event_count
    }

    pub fn create_touch_event(&self, sequence_id: u64, touch_type: TouchType, x: f32, y: f32) -> TouchEvent {
        TouchEvent {
            sequence_id,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            touch_type,
            x,
            y,
            pressure: 1.0,
            touch_major: 1.0,
            pointer_id: 0,
        }
    }

    pub fn create_key_event(&self, sequence_id: u64, key_type: KeyEventType, key_code: i32, text: Option<String>) -> KeyEvent {
        KeyEvent {
            sequence_id,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            key_type,
            key_code,
            text,
        }
    }
}

impl Default for InputDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Touch event builder for convenience
pub struct TouchEventBuilder {
    sequence_id: u64,
    touch_type: TouchType,
    x: f32,
    y: f32,
    pressure: f32,
}

impl TouchEventBuilder {
    pub fn new() -> Self {
        Self {
            sequence_id: 0,
            touch_type: TouchType::Down,
            x: 0.0,
            y: 0.0,
            pressure: 1.0,
        }
    }

    pub fn sequence_id(mut self, id: u64) -> Self {
        self.sequence_id = id;
        self
    }

    pub fn touch_type(mut self, t: TouchType) -> Self {
        self.touch_type = t;
        self
    }

    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn pressure(mut self, p: f32) -> Self {
        self.pressure = p;
        self
    }

    pub fn build(self) -> TouchEvent {
        TouchEvent {
            sequence_id: self.sequence_id,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            touch_type: self.touch_type,
            x: self.x,
            y: self.y,
            pressure: self.pressure,
            touch_major: 1.0,
            pointer_id: 0,
        }
    }
}

impl Default for TouchEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_dispatcher_new() {
        let dispatcher = InputDispatcher::new();
        assert_eq!(dispatcher.event_count(), 0);
    }

    #[test]
    fn test_dispatch_touch_event() {
        let mut dispatcher = InputDispatcher::new();
        let mut handler = DefaultInputHandler::new();

        let touch = TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            touch_type: TouchType::Down,
            x: 100.0,
            y: 200.0,
            pressure: 0.8,
            touch_major: 5.0,
            pointer_id: 0,
        };
        let event = InputEvent::Touch(touch);

        dispatcher.dispatch_to_handler(&event, &mut handler).unwrap();
        assert_eq!(dispatcher.event_count(), 1);
        assert_eq!(handler.touch_count(), 1);
        assert_eq!(handler.key_count(), 0);
    }

    #[test]
    fn test_dispatch_key_event() {
        let mut dispatcher = InputDispatcher::new();
        let mut handler = DefaultInputHandler::new();

        let key = KeyEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            key_type: KeyEventType::Down,
            key_code: 65,
            text: None,
        };
        let event = InputEvent::Key(key);

        dispatcher.dispatch_to_handler(&event, &mut handler).unwrap();
        assert_eq!(dispatcher.event_count(), 1);
        assert_eq!(handler.key_count(), 1);
        assert_eq!(handler.touch_count(), 0);
    }

    #[test]
    fn test_multiple_events() {
        let mut dispatcher = InputDispatcher::new();
        let mut handler = DefaultInputHandler::new();

        for i in 1..=5 {
            let touch = TouchEvent {
                sequence_id: i,
                timestamp_ms: 1000 + i,
                touch_type: TouchType::Move,
                x: i as f32 * 10.0,
                y: i as f32 * 20.0,
                pressure: 1.0,
                touch_major: 5.0,
                pointer_id: 0,
            };
            let event = InputEvent::Touch(touch);
            dispatcher.dispatch_to_handler(&event, &mut handler).unwrap();
        }

        for i in 1..=3 {
            let key = KeyEvent {
                sequence_id: i,
                timestamp_ms: 2000 + i,
                key_type: KeyEventType::Down,
                key_code: 65 + i as i32,
                text: None,
            };
            let event = InputEvent::Key(key);
            dispatcher.dispatch_to_handler(&event, &mut handler).unwrap();
        }

        assert_eq!(dispatcher.event_count(), 8);
        assert_eq!(handler.touch_count(), 5);
        assert_eq!(handler.key_count(), 3);
    }

    #[test]
    fn test_dispatch_basic() {
        let mut dispatcher = InputDispatcher::new();

        let touch = TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            touch_type: TouchType::Up,
            x: 100.0,
            y: 100.0,
            pressure: 0.0,
            touch_major: 1.0,
            pointer_id: 0,
        };
        let event = InputEvent::Touch(touch);

        dispatcher.dispatch(&event).unwrap();
        assert_eq!(dispatcher.event_count(), 1);
    }

    #[test]
    fn test_create_touch_event() {
        let dispatcher = InputDispatcher::new();
        let event = dispatcher.create_touch_event(42, TouchType::Down, 320.0, 480.0);

        assert_eq!(event.sequence_id, 42);
        assert_eq!(event.touch_type, TouchType::Down);
        assert_eq!(event.x, 320.0);
        assert_eq!(event.y, 480.0);
        assert!(event.timestamp_ms > 0);
    }

    #[test]
    fn test_create_key_event() {
        let dispatcher = InputDispatcher::new();
        let event = dispatcher.create_key_event(10, KeyEventType::Text, 0, Some("Hello".to_string()));

        assert_eq!(event.sequence_id, 10);
        assert_eq!(event.key_type, KeyEventType::Text);
        assert_eq!(event.key_code, 0);
        assert_eq!(event.text, Some("Hello".to_string()));
        assert!(event.timestamp_ms > 0);
    }

    #[test]
    fn test_touch_event_builder() {
        let event = TouchEventBuilder::new()
            .sequence_id(42)
            .touch_type(TouchType::Down)
            .position(500.0, 300.0)
            .pressure(0.9)
            .build();

        assert_eq!(event.sequence_id, 42);
        assert_eq!(event.touch_type, TouchType::Down);
        assert_eq!(event.x, 500.0);
        assert_eq!(event.y, 300.0);
        assert_eq!(event.pressure, 0.9);
    }

    #[test]
    fn test_default_input_handler() {
        let mut handler = DefaultInputHandler::new();

        let touch = TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            touch_type: TouchType::Down,
            x: 10.0,
            y: 20.0,
            pressure: 1.0,
            touch_major: 1.0,
            pointer_id: 0,
        };
        handler.handle_touch(&touch).unwrap();

        let key = KeyEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            key_type: KeyEventType::Down,
            key_code: 32,
            text: None,
        };
        handler.handle_key(&key).unwrap();

        assert_eq!(handler.touch_count(), 1);
        assert_eq!(handler.key_count(), 1);
    }

    #[test]
    fn test_touch_type_variants() {
        let types = [TouchType::Down, TouchType::Move, TouchType::Up, TouchType::Cancel];
        for t in types.iter() {
            let touch = TouchEvent {
                sequence_id: 1,
                timestamp_ms: 1000,
                touch_type: *t,
                x: 0.0,
                y: 0.0,
                pressure: 1.0,
                touch_major: 1.0,
                pointer_id: 0,
            };
            // Verify the event is valid and can be processed
            let mut handler = DefaultInputHandler::new();
            let event = InputEvent::Touch(touch);
            let mut dispatcher = InputDispatcher::new();
            dispatcher.dispatch_to_handler(&event, &mut handler).unwrap();
        }
    }

    #[test]
    fn test_key_event_type_variants() {
        let types = [KeyEventType::Down, KeyEventType::Up, KeyEventType::Text];
        for t in types.iter() {
            let key = KeyEvent {
                sequence_id: 1,
                timestamp_ms: 1000,
                key_type: *t,
                key_code: 65,
                text: match t {
                    KeyEventType::Text => Some("A".to_string()),
                    _ => None,
                },
            };
            let mut handler = DefaultInputHandler::new();
            let event = InputEvent::Key(key);
            let mut dispatcher = InputDispatcher::new();
            dispatcher.dispatch_to_handler(&event, &mut handler).unwrap();
        }
    }

    #[test]
    fn test_touch_sequence_flow() {
        let mut dispatcher = InputDispatcher::new();
        let mut handler = DefaultInputHandler::new();

        // Simulate a touch sequence: down -> move -> up
        let positions = [(100.0, 200.0), (110.0, 205.0), (120.0, 210.0)];

        // Touch down
        let down = dispatcher.create_touch_event(1, TouchType::Down, positions[0].0, positions[0].1);
        dispatcher.dispatch_to_handler(&InputEvent::Touch(down), &mut handler).unwrap();

        // Multiple moves
        for (i, pos) in positions[1..].iter().enumerate() {
            let move_event = dispatcher.create_touch_event((i + 2) as u64, TouchType::Move, pos.0, pos.1);
            dispatcher.dispatch_to_handler(&InputEvent::Touch(move_event), &mut handler).unwrap();
        }

        // Touch up
        let up = dispatcher.create_touch_event(positions.len() as u64 + 1, TouchType::Up, positions[2].0, positions[2].1);
        dispatcher.dispatch_to_handler(&InputEvent::Touch(up), &mut handler).unwrap();

        assert_eq!(handler.touch_count(), positions.len() as u64 + 1);
        assert_eq!(dispatcher.event_count(), positions.len() as u64 + 1);
    }
}
