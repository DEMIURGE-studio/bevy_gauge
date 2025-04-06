
// StatPath struct to handle path parsing and avoid repetitive string operations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatPath {
    pub(crate) path: String,
    pub(crate) owner: Option<String>,
    pub(crate) segments: Vec<String>,
}

impl StatPath {
    pub fn parse(string: &str) -> Self {
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

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn to_string(&self) -> String {
        self.path.clone()
    }

    pub fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    pub fn segments(&self) -> Vec<&str> {
        self.segments.iter().map(|s| s.as_str()).collect()
    }

    pub fn base(&self) -> Option<&str> {
        self.segments.first().map(|s| s.as_str())
    }

    pub fn with_owner(stat_path: &str, owner_prefix: &str) -> String {
        format!("{}@{}", owner_prefix, stat_path)
    }
}