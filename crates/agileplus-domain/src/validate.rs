//! Dependency-free validation helpers.
//!
//! Local, zero-dependency mirror of the small subset of `agileplus-validate`
//! used by the domain aggregates. Keeps `agileplus-domain` free of adapter
//! dependencies (see CI "Domain Zero-Dep Lint").

/// Require a non-empty (post-trim) name.
pub fn name_required(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("name must not be empty".to_string());
    }
    Ok(())
}

/// Require a lowercase kebab-case slug of ASCII letters, digits, and hyphens.
pub fn slug_format(slug: &str) -> Result<(), String> {
    if slug.is_empty() {
        return Err("slug must not be empty".to_string());
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err("slug must not start or end with '-'".to_string());
    }
    if slug.contains("--") {
        return Err("slug must not contain consecutive hyphens".to_string());
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("slug must contain only lowercase letters, digits, and hyphens".to_string());
    }
    Ok(())
}
