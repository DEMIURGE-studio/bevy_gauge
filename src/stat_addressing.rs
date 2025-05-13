#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatPath<'a> {
    /// The original full string path.
    pub full_path: &'a str,
    /// The primary name of the stat (e.g., "Strength"). First segment.
    pub name: &'a str,
    /// An optional single part of the path immediately following the name
    /// (e.g., "Added" in "Strength.Added" or "Strength.Added.123").
    /// This is None if the segment after 'name' is a numerical tag.
    pub part: Option<&'a str>,
    /// Optional u32 tag. This can be the segment after 'name' (if numerical)
    /// or the segment after 'part' (if numerical).
    pub tag: Option<u32>,
    /// An optional target entity or context.
    pub target: Option<&'a str>,
}

/// Parses a segment string to determine if it represents a u32 number.
fn parse_segment_as_numerical_tag(segment: &str) -> Option<u32> {
    // Trim whitespace to be a bit more lenient, though typically paths don't have spaces.
    segment.trim().parse::<u32>().ok()
}

impl<'a> StatPath<'a> {
    /// Parses a string slice into a `StatPath`.
    ///
    /// Parsing Logic:
    /// 1. Handles `@target` prefix.
    /// 2. `name` is the first segment of the remaining path.
    /// 3. If a second segment (`s1`) exists:
    ///    a. If `s1` is a numerical tag, it becomes `tag`; `part` is `None`.
    ///    b. If `s1` is not a numerical tag, it becomes `part`.
    ///       Then, if a third segment (`s2`) exists and is a numerical tag, it becomes `tag`.
    /// 4. Any further segments are ignored by this structure.
    pub fn parse(s: &'a str) -> Self {
        let full_path = s;
        let mut target_val: Option<&'a str> = None;
        let mut path_to_parse: &'a str = s;

        if let Some((base_candidate, source_candidate)) = path_to_parse.rsplit_once('@') {
            if !source_candidate.is_empty() {
                target_val = Some(source_candidate);
            }
            path_to_parse = base_candidate;
        }

        if path_to_parse.is_empty() && target_val.is_some() {
            // This case handles if the input was just "@SomeTarget"
            // or if after splitting, base_candidate was empty (e.g. "@Target")
            return Self {
                full_path,
                name: "", // No actual stat name part
                part: None,
                tag: None,
                target: target_val,
            };
        } else if path_to_parse.is_empty() {
            // Input was completely empty string or only "@"
            return Self {
                full_path,
                name: "",
                part: None,
                tag: None,
                target: None, // or target_val which would be None if path_to_parse is empty from just "@"
            };
        }

        let all_segments: Vec<&'a str> = path_to_parse.split('.').collect();

        let mut name_val: &'a str = "";
        let mut part_val: Option<&'a str> = None;
        let mut tag_val: Option<u32> = None;

        // name_val is the first segment. If path_to_parse was ".", all_segments is ["", ""].
        // If path_to_parse was "", all_segments is [""] (but caught by is_empty above).
        if let Some(first_segment) = all_segments.get(0) {
            name_val = first_segment;
        }
        // else name_val remains "", which is correct if all_segments was unexpectedly empty
        // despite path_to_parse not being empty (highly unlikely with split).

        if let Some(s1) = all_segments.get(1).cloned() { // Segment after name
            if let Some(parsed_tag) = parse_segment_as_numerical_tag(s1) {
                tag_val = Some(parsed_tag);
                // part_val remains None as s1 was consumed as a tag.
                // Any s2 (third segment) is ignored if s1 is a tag.
            } else {
                // s1 is not a numerical tag, so it's a part.
                part_val = Some(s1);
                if let Some(s2) = all_segments.get(2).cloned() { // Segment after part
                    if let Some(parsed_tag_s2) = parse_segment_as_numerical_tag(s2) {
                        tag_val = Some(parsed_tag_s2);
                    }
                    // Else s2 is not a numerical tag; it's ignored (as part is only s1).
                }
            }
        }
        // If only one segment (name_val), part_val and tag_val remain None.

        Self {
            full_path,
            name: name_val,
            part: part_val,
            tag: tag_val,
            target: target_val,
        }
    }

    // --- Accessor Methods (remain the same) ---
    pub fn full_path(&self) -> &'a str { self.full_path }
    pub fn name(&self) -> &'a str { self.name }
    pub fn part(&self) -> Option<&'a str> { self.part }
    pub fn tag(&self) -> Option<u32> { self.tag }
    pub fn target(&self) -> Option<&'a str> { self.target }
    pub fn has_target(&self) -> bool { self.target.is_some() }
    pub fn has_tags(&self) -> bool { self.tag.is_some() }

    pub fn without_target_as_string(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.name.to_string());
        if let Some(p) = self.part { parts.push(p.to_string()); }
        if let Some(t) = self.tag { parts.push(t.to_string()); }
        parts.join(".")
    }
}

impl<'a> ToString for StatPath<'a> {
    fn to_string(&self) -> String {
        self.full_path.to_string()
    }
}