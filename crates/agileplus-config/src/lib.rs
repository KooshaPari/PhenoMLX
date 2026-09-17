// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared config-builder macro for AgilePlus crates.
//!
//! [`config_builder!`] generates a struct with a `Default` impl and ergonomic
//! `with_<field>` builder methods.
//!
//! Field kind tags (prefix each field declaration):
//! - `(str)` — `String` field, setter accepts `impl Into<String>`
//! - `(opt_str)` — `Option<String>` field, setter wraps value in `Some(val.into())`
//! - `(val)` — all other types, setter takes the field type directly

// Re-export paste so that config_builder! expansions in downstream crates work
// without paste as a direct dependency of each consumer.
#[doc(hidden)]
pub use paste;

/// Generate a struct with builder-style `with_*` setters and a `Default` impl.
///
/// Syntax:
/// ```text
/// config_builder! {
///     #[derive(Clone, Debug)]
///     pub struct Foo {
///         (str)     pub name: String = "default".to_string(),
///         (opt_str) pub token: Option<String> = None,
///         (val)     pub count: u32 = 0,
///     }
/// }
/// ```
#[macro_export]
macro_rules! config_builder {
    (
        $(#[$attr:meta])*
        $vis:vis struct $Name:ident {
            $(
                $(#[$fattr:meta])*
                ($kind:tt)
                $fvis:vis $field:ident : $Type:ty = $default:expr
            ),* $(,)?
        }
    ) => {
        $(#[$attr])*
        $vis struct $Name {
            $(
                $(#[$fattr])*
                $fvis $field: $Type,
            )*
        }

        impl Default for $Name {
            fn default() -> Self {
                Self { $( $field: $default, )* }
            }
        }

        impl $Name {
            $( $crate::config_builder!(@setter $kind $field $Type); )*
        }
    };

    // String field → impl Into<String>
    (@setter str $field:ident $Type:ty) => {
        $crate::paste::paste! {
            pub fn [<with_ $field>](mut self, val: impl Into<String>) -> Self {
                self.$field = val.into();
                self
            }
        }
    };

    // Option<String> field → Some(impl Into<String>)
    (@setter opt_str $field:ident $Type:ty) => {
        $crate::paste::paste! {
            pub fn [<with_ $field>](mut self, val: impl Into<String>) -> Self {
                self.$field = Some(val.into());
                self
            }
        }
    };

    // All other types — direct assignment
    (@setter val $field:ident $Type:ty) => {
        $crate::paste::paste! {
            pub fn [<with_ $field>](mut self, val: $Type) -> Self {
                self.$field = val;
                self
            }
        }
    };
}

#[cfg(test)]
mod tests {
    // ── CacheConfig-like struct ──────────────────────────────────────────────
    crate::config_builder! {
        /// Cache configuration (macro smoke-test).
        #[derive(Clone, Debug, PartialEq)]
        pub struct CacheConfig {
            (str) pub host: String = "localhost".to_string(),
            (val) pub port: u16 = 6379,
            (val) pub pool_size: u32 = 16,
            (val) pub default_ttl_secs: u64 = 3600,
            (val) pub connection_timeout_secs: u64 = 5,
        }
    }

    #[test]
    fn cache_default_values() {
        let c = CacheConfig::default();
        assert_eq!(c.host, "localhost");
        assert_eq!(c.port, 6379);
        assert_eq!(c.pool_size, 16);
        assert_eq!(c.default_ttl_secs, 3600);
        assert_eq!(c.connection_timeout_secs, 5);
    }

    #[test]
    fn cache_with_pool_size_sets_field() {
        let c = CacheConfig::default().with_pool_size(32);
        assert_eq!(c.pool_size, 32);
        assert_eq!(c.host, "localhost"); // unaffected
    }

    #[test]
    fn cache_with_default_ttl_secs_sets_field() {
        let c = CacheConfig::default().with_default_ttl_secs(7200);
        assert_eq!(c.default_ttl_secs, 7200);
    }

    #[test]
    fn cache_with_connection_timeout_secs_sets_field() {
        let c = CacheConfig::default().with_connection_timeout_secs(10);
        assert_eq!(c.connection_timeout_secs, 10);
    }

    #[test]
    fn cache_with_host_accepts_str_slice() {
        let c = CacheConfig::default().with_host("redis-host");
        assert_eq!(c.host, "redis-host");
    }

    #[test]
    fn cache_with_port_sets_value() {
        let c = CacheConfig::default().with_port(6380);
        assert_eq!(c.port, 6380);
    }

    #[test]
    fn cache_builders_chainable() {
        let c = CacheConfig::default()
            .with_pool_size(8)
            .with_default_ttl_secs(60);
        assert_eq!(c.pool_size, 8);
        assert_eq!(c.default_ttl_secs, 60);
    }

    // ── NatsConfig-like struct ───────────────────────────────────────────────
    crate::config_builder! {
        /// NATS configuration (macro smoke-test).
        #[derive(Clone, Debug, PartialEq)]
        pub struct NatsConfig {
            (str)     pub url: String = "nats://localhost:4222".to_string(),
            (opt_str) pub auth_token: Option<String> = None,
            (str)     pub subject_prefix: String = "agileplus".to_string(),
            (val)     pub max_payload: usize = 1_048_576,
        }
    }

    #[test]
    fn nats_default_values() {
        let c = NatsConfig::default();
        assert_eq!(c.url, "nats://localhost:4222");
        assert!(c.auth_token.is_none());
        assert_eq!(c.subject_prefix, "agileplus");
        assert_eq!(c.max_payload, 1_048_576);
    }

    #[test]
    fn nats_with_auth_token_wraps_some() {
        let c = NatsConfig::default().with_auth_token("secret");
        assert_eq!(c.auth_token, Some("secret".to_string()));
    }

    #[test]
    fn nats_with_subject_prefix_sets_field() {
        let c = NatsConfig::default().with_subject_prefix("myapp");
        assert_eq!(c.subject_prefix, "myapp");
    }

    #[test]
    fn nats_with_max_payload_sets_field() {
        let c = NatsConfig::default().with_max_payload(4096);
        assert_eq!(c.max_payload, 4096);
    }

    #[test]
    fn nats_builders_are_chainable() {
        let c = NatsConfig::default()
            .with_url("nats://prod:4222")
            .with_auth_token("tok")
            .with_max_payload(512);
        assert_eq!(c.url, "nats://prod:4222");
        assert_eq!(c.auth_token, Some("tok".to_string()));
        assert_eq!(c.max_payload, 512);
    }
}
