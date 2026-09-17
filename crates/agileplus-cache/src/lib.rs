//! agileplus-cache — caching layer for projections and rate limiting.

pub mod config;
#[cfg(feature = "redis")]
pub mod limiter;
pub mod projection;
pub mod store;

#[cfg(feature = "redis")]
pub mod health;
#[cfg(feature = "redis")]
pub mod pool;

pub use config::CacheConfig;
pub use projection::ProjectionCache;
pub use store::{CacheError, CacheStore, InMemoryCacheStore};

#[cfg(feature = "redis")]
pub use store::RedisCacheStore;
