use serde::{Deserialize, Serialize};  
  
/// Canonical trace-link type between artifacts.  
/// Aligned with `RelType` in `agileplus-graph`.  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]  
pub enum TraceLinkType {  
    Satisfies,  
    Verifies,  
    Implements,  
    DerivesFrom,  
    ConflictsWith,  
    Duplicates,  
}  
  
impl TraceLinkType {  
    pub fn as_str(&self) -> &'static str {  
        match self {  
            TraceLinkType::Satisfies => "Satisfies",  
            TraceLinkType::Verifies => "Verifies",  
            TraceLinkType::Implements => "Implements",  
            TraceLinkType::DerivesFrom => "DerivesFrom",  
            TraceLinkType::ConflictsWith => "ConflictsWith",  
            TraceLinkType::Duplicates => "Duplicates",  
        }  
    }  
} 
  
/// Generic artifact reference.  
/// Aligned with `Node` in `agileplus-graph` (id + type/kind).  
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]  
pub struct Artifact {  
    pub id: String,  
    pub kind: String,  
}  
  
impl Artifact {  
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {  
        Self {  
            id: id.into(),  
            kind: kind.into(),  
        }  
    }  
}  
  
/// Requirement artifact with lifecycle status.  
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]  
pub struct Requirement {  
    pub id: String,  
    pub status: String,  
}  
  
impl Requirement {  
    pub fn new(id: impl Into<String>, status: impl Into<String>) -> Self {  
        Self {  
            id: id.into(),  
            status: status.into(),  
        }  
    }  
}  
  
/// Directed trace link between two artifacts.  
/// Aligned with `Relationship` in `agileplus-graph`.  
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]  
pub struct TraceLink {  
    pub source: String,  
    pub target: String,  
    pub link_type: TraceLinkType,  
}  
  
impl TraceLink {  
    pub fn new(source: impl Into<String>, target: impl Into<String>, link_type: TraceLinkType) -> Self {  
        Self {  
            source: source.into(),  
            target: target.into(),  
            link_type,  
        }  
    }  
}  
  
/// Coverage state for a requirement or artifact.  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]  
pub enum CoverageState {  
    Covered,  
    Partial,  
    Uncovered,  
    Orphaned,  
}  
  
impl CoverageState {  
    pub fn as_str(&self) -> &'static str {  
        match self {  
            CoverageState::Covered => "Covered",  
            CoverageState::Partial => "Partial",  
            CoverageState::Uncovered => "Uncovered",  
            CoverageState::Orphaned => "Orphaned",  
        }  
    }  
} 
  
#[cfg(test)]  
mod tests {  
    use super::*;  
    use serde_json;  
  
    #[test]  
    fn round_trip_trace_link_type() {  
        for variant in [  
            TraceLinkType::Satisfies,  
            TraceLinkType::Verifies,  
            TraceLinkType::Implements,  
            TraceLinkType::DerivesFrom,  
            TraceLinkType::ConflictsWith,  
            TraceLinkType::Duplicates,  
        ] {  
            let json = serde_json::to_string(&variant).unwrap();  
            let back: TraceLinkType = serde_json::from_str(&json).unwrap();  
            assert_eq!(variant, back);  
        }  
    }  
  
    #[test]  
    fn round_trip_artifact() {  
        let original = Artifact::new("ART-001", "requirement");  
        let json = serde_json::to_string(&original).unwrap();  
        let back: Artifact = serde_json::from_str(&json).unwrap();  
        assert_eq!(original, back);  
    }  
  
    #[test]  
    fn round_trip_requirement() {  
        let original = Requirement::new("REQ-42", "draft");  
        let json = serde_json::to_string(&original).unwrap();  
        let back: Requirement = serde_json::from_str(&json).unwrap();  
        assert_eq!(original, back);  
    }  
  
    #[test]  
    fn round_trip_trace_link() {  
        let original = TraceLink::new("SRC-1", "TGT-2", TraceLinkType::Verifies);  
        let json = serde_json::to_string(&original).unwrap();  
        let back: TraceLink = serde_json::from_str(&json).unwrap();  
        assert_eq!(original, back);  
    }  
  
    #[test]  
    fn round_trip_coverage_state() {  
        for variant in [  
            CoverageState::Covered,  
            CoverageState::Partial,  
            CoverageState::Uncovered,  
            CoverageState::Orphaned,  
        ] {  
            let json = serde_json::to_string(&variant).unwrap();  
            let back: CoverageState = serde_json::from_str(&json).unwrap();  
            assert_eq!(variant, back);  
        }  
    }  
} 
