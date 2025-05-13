use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatPath {
    // Base stat name (e.g., "Damage")
    pub name: String,
    // Optional part (e.g., "Added", "Increased")
    pub part: Option<String>,
    // Optional tag (e.g., 1, 3)
    pub tag: Option<u32>,
    // Optional source entity name (e.g., "Source", "Parent")
    pub target: Option<String>,
    // Original full path string
    pub full_path: String,
}

impl StatPath {
    pub const SOURCE_SEPARATOR: &'static str = "@";

    pub fn parse(path_str: &str) -> Self {
        let full_path = path_str.to_string();
        let mut source_name: Option<String> = None; // Initialize to None
        let mut base_path_str: &str = path_str; // Initialize to full path_str

        if let Some((base_val, source_val)) = path_str.rsplit_once(Self::SOURCE_SEPARATOR) {
            source_name = Some(source_val.to_string());
            base_path_str = base_val;
        } else {
            // source_name remains None, base_path_str remains path_str
        }
        
        // Parse the base path (Stat.Part.Tag)
        let mut parts_iter = base_path_str.split('.');
        let name = parts_iter.next().unwrap_or("").to_string();
        let part = parts_iter.next().map(|s| s.to_string());
        let tag = parts_iter.next().and_then(|s| s.parse::<u32>().ok());
        // Ignore further parts for now

        StatPath {
            name,
            part,
            tag,
            target: source_name, // Assign the parsed source name here
            full_path,
        }
    }

    // Reconstruct the path in the correct format
    pub fn to_string(&self) -> String {
        let mut base_path = String::new();
        base_path.push_str(&self.name);
        if let Some(part) = &self.part {
            base_path.push('.');
            base_path.push_str(part);
        }
        if let Some(tag) = self.tag {
            base_path.push('.');
            base_path.push_str(&tag.to_string());
        }

        // Append the source separator and name if present
        if let Some(target) = &self.target {
            base_path.push_str(Self::SOURCE_SEPARATOR);
            base_path.push_str(target);
        }
        
        base_path // Return the reconstructed path
    }

    // Keep the display implementation consistent with full_path
    impl fmt::Display for StatPath {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.full_path)
        }
    }
} 