
// StatPath struct to handle path parsing and avoid repetitive string operations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatPath {
    pub(crate) path: String,
    pub(crate) owner: Option<String>,
    pub(crate) segments: Vec<String>,
}

impl StatPath {
    pub(crate) fn parse(string: &str) -> Self {
        let (owner, segments) = if string.contains('@') {
            let parts: Vec<&str> = string.split('@').collect();
            let owner = Some(parts[0].to_string());
            let segments = parts[1].split('.').map(|s| s.to_string()).collect();
            (owner, segments)
        } else {
            let segments = string.split('.').map(|s| s.to_string()).collect();
            (None, segments)
        };
        Self { 
            path: string.to_string(), 
            owner, 
            segments,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.segments.len()
    }

    pub(crate) fn to_string(&self) -> String {
        self.path.clone()
    }

    pub(crate) fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    pub(crate) fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    pub(crate) fn segments(&self) -> Vec<&str> {
        self.segments.iter().map(|s| s.as_str()).collect()
    }

    pub(crate) fn base(&self) -> Option<&str> {
        self.segments.first().map(|s| s.as_str())
    }

    pub(crate) fn with_owner(path: &str, owner_prefix: &str) -> String {
        format!("{}@{}", owner_prefix, path)
    }
}