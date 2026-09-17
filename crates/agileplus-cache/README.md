# agileplus-cache

Cache, projection cache, rate limiting, pooling, and health abstractions.

## Public API Index

- `CacheConfig`, `CacheHealth`, `CacheHealthChecker`.
- `RateLimiter`, `CachePool`, `ProjectionCache`.
- Store API: `CacheStore`, `CacheError`, `InMemoryCacheStore`, `RedisCacheStore`.

## Validation

```bash
cargo test -p agileplus-cache
```

