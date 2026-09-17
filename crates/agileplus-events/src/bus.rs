//! In-process pub/sub event bus backed by `tokio::sync::broadcast`.
//!
//! Use this when multiple subsystems need to react to a domain event in
//! real time (e.g. webhook fan-out, cockpit updates, plane sync). The
//! append-only `EventStore` in this crate handles durability + replay;
//! the `EventBus` handles live fan-out.
//!
//! # Example
//!
//! ```no_run
//! use agileplus_events::bus::{EventBus, DomainEvent};
//! # async fn run() {
//! let bus = EventBus::new(64);
//! let mut sub = bus.subscribe();
//! tokio::spawn(async move {
//!     while let Ok(ev) = sub.recv().await {
//!         eprintln!("got: {ev:?}");
//!     }
//! });
//! bus.publish(DomainEvent::FeatureCreated { id: 1 }).await.unwrap();
//! # }
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::SendError;

/// Domain event variants that flow through the bus. Keep this small and
/// stable — every consumer has to be able to decode every variant, so
/// prefer a flat hierarchy of clear, simple payloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DomainEvent {
    FeatureCreated {
        id: i64,
    },
    FeatureStateChanged {
        id: i64,
        from: String,
        to: String,
    },
    CycleStarted {
        cycle_id: i64,
        module_id: i64,
    },
    CycleEnded {
        cycle_id: i64,
    },
    WorkPackageLinked {
        work_package_id: i64,
        feature_id: i64,
    },
    UserLoggedIn {
        user_id: String,
    },
    PlaneWebhookReceived {
        issue_id: String,
        action: String,
    },
    Custom {
        name: String,
        payload: serde_json::Value,
    },
}

/// Multi-producer, multi-consumer in-process event bus.
///
/// Cheap to clone (wraps an `Arc`) and safe to share across tasks.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<broadcast::Sender<DomainEvent>>,
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("receiver_count", &self.inner.receiver_count())
            .finish()
    }
}

impl EventBus {
    /// Create a bus with the given per-subscriber buffer size. A subscriber
    /// that falls behind by more than `capacity` messages will see
    /// `RecvError::Lagged` and skip ahead.
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity.max(1));
        Self {
            inner: Arc::new(tx),
        }
    }

    /// Publish an event to every subscriber. Returns the number of
    /// receivers that accepted it. Slow subscribers cause lagged events
    /// to be dropped, never blocking the publisher.
    pub fn publish(&self, event: DomainEvent) -> Result<usize, SendError<DomainEvent>> {
        self.inner.send(event)
    }

    /// Async convenience wrapper around [`Self::publish`].
    pub async fn publish_async(
        &self,
        event: DomainEvent,
    ) -> Result<usize, SendError<DomainEvent>> {
        self.publish(event)
    }

    /// Get a new subscription handle.
    pub fn subscribe(&self) -> EventSubscriber {
        EventSubscriber {
            inner: self.inner.subscribe(),
        }
    }

    /// Current number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.inner.receiver_count()
    }
}

/// A handle to the event stream. Each subscriber receives every event
/// independently. Not `Clone` — receivers must be obtained from
/// `EventBus::subscribe()`.
pub struct EventSubscriber {
    inner: broadcast::Receiver<DomainEvent>,
}

impl EventSubscriber {
    /// Receive the next event, awaiting if necessary.
    ///
    /// Returns:
    /// - `Ok(event)` for a normal event
    /// - `Err(RecvError::Lagged(skipped))` if the subscriber fell behind
    ///   and skipped events
    /// - `Err(RecvError::Closed)` if the bus has been dropped
    pub async fn recv(&mut self) -> Result<DomainEvent, broadcast::error::RecvError> {
        self.inner.recv().await
    }

    /// Non-blocking variant. Returns `None` if no event is currently
    /// buffered.
    pub fn try_recv(&mut self) -> Option<Result<DomainEvent, broadcast::error::RecvError>> {
        match self.inner.try_recv() {
            Ok(ev) => Some(Ok(ev)),
            Err(broadcast::error::TryRecvError::Empty) => None,
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                Some(Err(broadcast::error::RecvError::Lagged(n)))
            }
            Err(broadcast::error::TryRecvError::Closed) => {
                Some(Err(broadcast::error::RecvError::Closed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn publish_delivers_to_subscriber() {
        let bus = EventBus::new(8);
        let mut sub = bus.subscribe();

        bus.publish(DomainEvent::FeatureCreated { id: 42 }).unwrap();

        let ev = tokio::time::timeout(Duration::from_millis(100), sub.recv())
            .await
            .expect("no timeout")
            .expect("no recv error");
        assert_eq!(ev, DomainEvent::FeatureCreated { id: 42 });
    }

    #[tokio::test]
    async fn multiple_subscribers_each_receive() {
        let bus = EventBus::new(8);
        let mut a = bus.subscribe();
        let mut b = bus.subscribe();

        bus.publish(DomainEvent::CycleStarted {
            cycle_id: 1,
            module_id: 2,
        })
        .unwrap();

        let ev_a = a.recv().await.unwrap();
        let ev_b = b.recv().await.unwrap();
        assert_eq!(ev_a, ev_b);
    }

    #[tokio::test]
    async fn subscriber_count_reflects_handles() {
        let bus = EventBus::new(4);
        assert_eq!(bus.subscriber_count(), 0);
        let _s1 = bus.subscribe();
        let _s2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }

    #[tokio::test]
    async fn try_recv_returns_none_when_empty() {
        let bus = EventBus::new(4);
        let mut sub = bus.subscribe();
        assert!(sub.try_recv().is_none());
    }

    #[tokio::test]
    async fn serde_round_trip_on_event() {
        let ev = DomainEvent::FeatureStateChanged {
            id: 7,
            from: "draft".into(),
            to: "review".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: DomainEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
    }
}