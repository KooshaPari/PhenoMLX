use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub projects_created: usize,
    pub projects_updated: usize,
    pub modules_created: usize,
    pub modules_updated: usize,
    pub features_created: usize,
    pub features_updated: usize,
    pub cycles_created: usize,
    pub cycles_updated: usize,
    pub work_packages_created: usize,
    pub work_packages_updated: usize,
    pub module_links_created: usize,
    pub cycle_links_created: usize,
    pub artifacts_written: usize,
    pub audits_written: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_report_has_all_zero_counts() {
        let report = ImportReport::default();
        assert_eq!(report.projects_created, 0);
        assert_eq!(report.features_created, 0);
        assert_eq!(report.work_packages_created, 0);
    }

    #[test]
    fn report_increments_are_reflected() {
        let mut report = ImportReport::default();
        report.features_created += 3;
        report.work_packages_created += 7;
        assert_eq!(report.features_created, 3);
        assert_eq!(report.work_packages_created, 7);
        assert_eq!(report.features_updated, 0);
    }

    #[test]
    fn report_roundtrips_through_serde_json() {
        let mut report = ImportReport::default();
        report.modules_created = 2;
        report.cycles_created = 1;
        let json = serde_json::to_string(&report).expect("serialize");
        let decoded: ImportReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.modules_created, 2);
        assert_eq!(decoded.cycles_created, 1);
        assert_eq!(decoded.artifacts_written, 0);
    }
}