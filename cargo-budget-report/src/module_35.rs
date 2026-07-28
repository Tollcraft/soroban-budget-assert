use std::collections::HashSet;

pub struct FilterSet {
    packages: HashSet<String>,
    functions: HashSet<String>,
}

impl FilterSet {
    pub fn new(packages: Vec<String>, functions: Vec<String>) -> Self {
        Self {
            packages: packages.into_iter().collect(),
            functions: functions.into_iter().collect(),
        }
    }

    pub fn has_packages(&self) -> bool {
        !self.packages.is_empty()
    }

    pub fn has_functions(&self) -> bool {
        !self.functions.is_empty()
    }

    pub fn wants_package(&self, name: &str) -> bool {
        !self.has_packages() || self.packages.contains(name)
    }

    pub fn wants_function(&self, name: &str) -> bool {
        !self.has_functions() || self.functions.contains(name)
    }

    pub fn unknown_packages<'a>(&'a self, cdylib_names: &HashSet<&'a str>) -> Vec<&'a str> {
        self.packages
            .iter()
            .filter(|p| !cdylib_names.contains(p.as_str()))
            .map(|p| p.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_filter_set_accepts_everything() {
        let fs = FilterSet::new(vec![], vec![]);
        assert!(!fs.has_packages());
        assert!(!fs.has_functions());
        assert!(fs.wants_package("any-package"));
        assert!(fs.wants_function("any-function"));
    }

    #[test]
    fn package_filter_accepts_matching_names() {
        let fs = FilterSet::new(vec!["my-contract".to_string(), "other".to_string()], vec![]);
        assert!(fs.has_packages());
        assert!(fs.wants_package("my-contract"));
        assert!(fs.wants_package("other"));
        assert!(!fs.wants_package("unknown"));
    }

    #[test]
    fn function_filter_accepts_matching_names() {
        let fs = FilterSet::new(vec![], vec!["do_work".to_string(), "ping".to_string()]);
        assert!(fs.has_functions());
        assert!(fs.wants_function("do_work"));
        assert!(fs.wants_function("ping"));
        assert!(!fs.wants_function("unknown"));
    }

    #[test]
    fn both_filters_work_together() {
        let fs = FilterSet::new(vec!["my-contract".to_string()], vec!["do_work".to_string()]);
        assert!(fs.has_packages());
        assert!(fs.has_functions());
        assert!(fs.wants_package("my-contract"));
        assert!(fs.wants_function("do_work"));
        assert!(!fs.wants_package("other"));
        assert!(!fs.wants_function("other"));
    }

    #[test]
    fn unknown_packages_returns_unrecognized() {
        let fs = FilterSet::new(vec!["known".to_string(), "unknown".to_string()], vec![]);
        let cdylib_set: HashSet<&str> = ["known"].into_iter().collect();
        let unknown = fs.unknown_packages(&cdylib_set);
        assert_eq!(unknown, vec!["unknown"]);
    }

    #[test]
    fn unknown_packages_empty_when_all_known() {
        let fs = FilterSet::new(vec!["known".to_string()], vec![]);
        let cdylib_set: HashSet<&str> = ["known"].into_iter().collect();
        let unknown = fs.unknown_packages(&cdylib_set);
        assert!(unknown.is_empty());
    }

    #[test]
    fn unknown_packages_empty_when_no_filters() {
        let fs = FilterSet::new(vec![], vec![]);
        let cdylib_set: HashSet<&str> = ["known"].into_iter().collect();
        let unknown = fs.unknown_packages(&cdylib_set);
        assert!(unknown.is_empty());
    }
}
