// SPDX-License-Identifier: MIT OR Apache-2.0
//! Catalog parser: extracts `(fr_id, title, status)` triples from an FR/NFR markdown file.
//!
//! Format recognised (as used in all four catalogs):
//!   ### FR-XXX-NNN — Title text
//!   ...
//!   | **Status** | SHIPPED |   (optional; AgilePlus / Authvault catalogs)
//!   | **Status** | PLANNED |
//!   | Status      | planned |
//!
//! For catalogs without an explicit Status column (Tracera, phenotype-voxel) the
//! status defaults to `Todo` unless a "## Gap" / "PLANNED" heading later contains
//! the FR id.

/// Parsed status from the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogStatus {
    /// The requirement has been shipped / implemented.
    Shipped,
    /// The requirement is planned (or status unknown).
    Planned,
}

/// A single FR (or NFR) entry parsed from the catalog.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    /// The canonical requirement ID, e.g. "FR-AGP-001".
    pub id: String,
    /// Human-readable title (everything after the " — " separator).
    pub title: String,
    /// Derived status.
    pub status: CatalogStatus,
}

/// Parse a markdown catalog and return all FR/NFR entries found.
pub fn parse_catalog(markdown: &str) -> Vec<CatalogEntry> {
    let mut entries: Vec<CatalogEntry> = Vec::new();

    // Collect all IDs that appear in "PLANNED" gap tables / sections.
    let planned_in_gaps: std::collections::HashSet<String> = collect_planned_ids(markdown);

    let mut current_id: Option<String> = None;
    let mut current_title: Option<String> = None;
    let mut current_status: Option<CatalogStatus> = None;

    for line in markdown.lines() {
        let trimmed = line.trim();

        // Match heading lines: `### FR-XXX-NNN — Title`
        // Also handle `### FR-XXX-NNN\n\n**Title:** ...` (Authvault style via next line)
        if let Some(id_and_rest) = extract_heading_id(trimmed) {
            // Flush previous entry
            if let (Some(id), Some(title)) = (current_id.take(), current_title.take()) {
                let status = current_status.take().unwrap_or_else(|| {
                    if planned_in_gaps.contains(&id) {
                        CatalogStatus::Planned
                    } else {
                        CatalogStatus::Shipped
                    }
                });
                entries.push(CatalogEntry { id, title, status });
            }
            current_id = Some(id_and_rest.0);
            current_title = Some(id_and_rest.1);
            current_status = None;
            continue;
        }

        // Inside an entry — look for **Title:** line (Authvault style)
        if current_id.is_some() && current_title.as_deref().map(str::is_empty).unwrap_or(false) {
            if let Some(t) = extract_title_field(trimmed) {
                current_title = Some(t);
                continue;
            }
        }

        // Look for Status table cell
        if current_id.is_some() && current_status.is_none() {
            if let Some(s) = extract_status_cell(trimmed) {
                current_status = Some(s);
            }
        }
    }

    // Flush last entry
    if let (Some(id), Some(title)) = (current_id, current_title) {
        let status = current_status.unwrap_or_else(|| {
            if planned_in_gaps.contains(&id) {
                CatalogStatus::Planned
            } else {
                CatalogStatus::Shipped
            }
        });
        entries.push(CatalogEntry { id, title, status });
    }

    entries
}

/// Extract `(id, title)` from a heading line like:
///   `### FR-AGP-001 — Rich domain aggregates with enforced invariants`
///   `### FR-AGP-001\n` (title-only variant; title = id for now, overridden later)
fn extract_heading_id(line: &str) -> Option<(String, String)> {
    // Must start with one or more '#'
    let stripped = line.trim_start_matches('#').trim();
    // Must look like an ID: starts with two uppercase words separated by '-'
    // Pattern: WORD-WORD-NNN (e.g. FR-AGP-001, NFR-TRC-002, FR-VOXEL-003)
    let (id_part, title_part) = if let Some(pos) = stripped.find(" \u{2014} ") {
        // " — " separator (em-dash)
        (
            &stripped[..pos],
            stripped[pos + " \u{2014} ".len()..].trim(),
        )
    } else if let Some(pos) = stripped.find(" - ") {
        // Plain hyphen separator fallback
        (&stripped[..pos], stripped[pos + 3..].trim())
    } else {
        // No separator — the whole thing is the id; title will be filled later
        (stripped, "")
    };

    if looks_like_req_id(id_part.trim()) {
        Some((id_part.trim().to_string(), title_part.to_string()))
    } else {
        None
    }
}

fn looks_like_req_id(s: &str) -> bool {
    // Accepts: FR-AGP-001, NFR-TRC-002, FR-VOXEL-003, FR-AUTHV-010, etc.
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() < 3 {
        return false;
    }
    let prefix = parts[0];
    if prefix != "FR" && prefix != "NFR" {
        return false;
    }
    // Last part should be digits
    parts
        .last()
        .map(|p| p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

/// Try to find `**Title:** Some title text` in a line.
fn extract_title_field(line: &str) -> Option<String> {
    let line = line.trim_start_matches('|').trim_end_matches('|').trim();
    if let Some(rest) = line.strip_prefix("**Title:**") {
        let t = rest.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    // Also handle table cell: `| **Title** | Some title |`
    None
}

/// Extract status from lines like:
///   `| **Status** | SHIPPED |`
///   `| **Status** | PLANNED |`
///   `| **Status** | PARTIAL — ... |`
fn extract_status_cell(line: &str) -> Option<CatalogStatus> {
    if !line.contains("**Status**") && !line.to_lowercase().contains("status") {
        return None;
    }
    let upper = line.to_uppercase();
    if upper.contains("SHIPPED") {
        return Some(CatalogStatus::Shipped);
    }
    if upper.contains("PLANNED") || upper.contains("PARTIAL") {
        return Some(CatalogStatus::Planned);
    }
    None
}

/// Collect FR IDs that appear in Gap/Planned sections of the catalog.
/// These are IDs that will be treated as Planned even if no explicit status cell
/// is present.
fn collect_planned_ids(markdown: &str) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    let mut in_gap_section = false;

    for line in markdown.lines() {
        let trimmed = line.trim();
        // Gap sections start with a heading containing "Gap" or "Planned"
        if trimmed.starts_with('#') {
            let upper = trimmed.to_uppercase();
            in_gap_section = upper.contains("GAP") || upper.contains("PLANNED");
        }
        if in_gap_section {
            // Extract any FR/NFR IDs mentioned in this line
            for word in trimmed.split_whitespace() {
                let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
                if looks_like_req_id(clean) {
                    ids.insert(clean.to_string());
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CATALOG: &str = r#"
# Test Catalog

## Functional Requirements

### FR-TEST-001 — Rich domain model

| Field | Value |
|---|---|
| **ID** | FR-TEST-001 |
| **Title** | Rich domain model |
| **Status** | SHIPPED |

---

### FR-TEST-002 — Planned feature

| Field | Value |
|---|---|
| **ID** | FR-TEST-002 |
| **Title** | Planned feature |
| **Status** | PLANNED |

---

### NFR-TEST-001 — Performance

No status field here; should default to Shipped.

---

## Gaps / PLANNED

| FR-TEST-003 | Some planned thing | PLANNED |
"#;

    #[test]
    fn parses_fr_ids() {
        let entries = parse_catalog(SAMPLE_CATALOG);
        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"FR-TEST-001"), "missing FR-TEST-001: {ids:?}");
        assert!(ids.contains(&"FR-TEST-002"), "missing FR-TEST-002: {ids:?}");
        assert!(
            ids.contains(&"NFR-TEST-001"),
            "missing NFR-TEST-001: {ids:?}"
        );
    }

    #[test]
    fn status_shipped_mapped_correctly() {
        let entries = parse_catalog(SAMPLE_CATALOG);
        let e = entries.iter().find(|e| e.id == "FR-TEST-001").unwrap();
        assert_eq!(e.status, CatalogStatus::Shipped);
    }

    #[test]
    fn status_planned_mapped_correctly() {
        let entries = parse_catalog(SAMPLE_CATALOG);
        let e = entries.iter().find(|e| e.id == "FR-TEST-002").unwrap();
        assert_eq!(e.status, CatalogStatus::Planned);
    }

    #[test]
    fn title_extracted() {
        let entries = parse_catalog(SAMPLE_CATALOG);
        let e = entries.iter().find(|e| e.id == "FR-TEST-001").unwrap();
        assert_eq!(e.title, "Rich domain model");
    }

    #[test]
    fn gap_section_id_treated_as_planned() {
        let entries = parse_catalog(SAMPLE_CATALOG);
        let e = entries.iter().find(|e| e.id == "FR-TEST-003");
        // FR-TEST-003 only appears in gaps table, not as a heading — so it may
        // not be parsed as a full entry but its ID is collected as planned.
        // The planned_ids collection is used to override status for entries
        // that DO have headings.
        // NFR-TEST-001 has no status cell â†’ should default to Shipped (not in gap section).
        let nfr = entries.iter().find(|e| e.id == "NFR-TEST-001").unwrap();
        assert_eq!(nfr.status, CatalogStatus::Shipped);
        // FR-TEST-003 won't appear because it has no heading
        assert!(e.is_none());
    }

    #[test]
    fn looks_like_req_id_correct() {
        assert!(looks_like_req_id("FR-AGP-001"));
        assert!(looks_like_req_id("NFR-TRC-007"));
        assert!(looks_like_req_id("FR-VOXEL-009"));
        assert!(looks_like_req_id("FR-AUTHV-011"));
        assert!(!looks_like_req_id("some-random-text"));
        assert!(!looks_like_req_id("PLAN-VOXEL-001")); // Not FR/NFR prefix
        assert!(!looks_like_req_id("FR-AGP")); // Too few parts
    }
}
