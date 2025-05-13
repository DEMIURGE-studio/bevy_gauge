/// Represents a parsed path to a stat, potentially including its name, part, tag, and target source.
///
/// Stat paths are strings used throughout the system to identify specific stats or their components.
/// Examples:
/// - `"Health"`: Refers to the base "Health" stat.
/// - `"Damage.base"`: Refers to the "base" part of the "Damage" stat.
/// - `"Damage.increased.123"`: Refers to the "increased" part of "Damage", specifically with tag `123`.
/// - `"Strength@Player"`: Refers to the "Strength" stat from the source aliased as "Player".
/// - `"Armor.base@EnemyTarget"`: Refers to the "base" part of "Armor" from source "EnemyTarget".
///
/// The `StatPath` struct holds these parsed components for easier access.
/// It is lifetime-parameterized (`\'a`) as it borrows string slices from the original path string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatPath<'a> {
    /// The original, unparsed string slice from which this `StatPath` was created.
    pub full_path: &'a str,
    /// The primary name of the stat (e.g., "Strength" in "Strength.base.1@Player").
    /// This is the first segment of the path before any `.` or `@`.
    pub name: &'a str,
    /// An optional part of the stat, typically following the name (e.g., "base" in "Damage.base").
    /// If the segment after the name is a numerical tag, `part` will be `None`.
    pub part: Option<&'a str>,
    /// An optional numerical tag associated with the stat or its part.
    /// This can be the segment directly after the `name` (if numerical), or after the `part` (if numerical).
    /// Example: `123` in `"Damage.increased.123"` or `"Buff.42"`.
    pub tag: Option<u32>,
    /// An optional source alias, indicating that the stat originates from a different entity or context.
    /// This is the segment following an `@` symbol (e.g., "Player" in `"Health@Player"`).
    pub target: Option<&'a str>,
}

/// Parses a segment string to determine if it represents a u32 number.
fn parse_segment_as_numerical_tag(segment: &str) -> Option<u32> {
    // Trim whitespace to be a bit more lenient, though typically paths don't have spaces.
    segment.trim().parse::<u32>().ok()
}

impl<'a> StatPath<'a> {
    /// Parses a string slice into a `StatPath`, dissecting it into its constituent components.
    ///
    /// The parsing logic is as follows:
    /// 1.  Checks for and extracts a target alias if an `@` symbol is present (e.g., `"StatName@TargetAlias"`).
    ///     The part before `@` becomes the path to parse for name, part, and tag.
    /// 2.  The `name` is the first segment of the path (before the first `.` if any).
    /// 3.  If a second segment exists after the `name`:
    ///     a.  If this segment can be parsed as a `u32`, it becomes the `tag`, and `part` remains `None`.
    ///     b.  Otherwise, this segment becomes the `part`.
    /// 4.  If a `part` was identified (from step 3b), and a third segment exists:
    ///     a.  If this third segment can be parsed as a `u32`, it becomes the `tag`.
    ///     b.  Otherwise, it is ignored for the purpose of `StatPath` fields.
    /// 5.  Any segments beyond these are not captured in the distinct fields of `StatPath` but are part of `full_path`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_gauge::prelude::StatPath; // Adjust import path as needed
    ///
    /// let p1 = StatPath::parse("Damage.base.10@Player");
    /// assert_eq!(p1.name, "Damage");
    /// assert_eq!(p1.part, Some("base"));
    /// assert_eq!(p1.tag, Some(10));
    /// assert_eq!(p1.target, Some("Player"));
    ///
    /// let p2 = StatPath::parse("Health.25");
    /// assert_eq!(p2.name, "Health");
    /// assert_eq!(p2.part, None);
    /// assert_eq!(p2.tag, Some(25));
    /// assert_eq!(p2.target, None);
    ///
    /// let p3 = StatPath::parse("Speed@MyCharacter");
    /// assert_eq!(p3.name, "Speed");
    /// assert_eq!(p3.part, None);
    /// assert_eq!(p3.tag, None);
    /// assert_eq!(p3.target, Some("MyCharacter"));
    ///
    /// let p4 = StatPath::parse("Mana");
    /// assert_eq!(p4.name, "Mana");
    /// assert_eq!(p4.part, None);
    /// assert_eq!(p4.tag, None);
    /// assert_eq!(p4.target, None);
    /// ```
    ///
    /// # Arguments
    ///
    /// * `s`: The string slice to parse.
    ///
    /// # Returns
    ///
    /// A `StatPath` instance representing the parsed components of the input string.
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

    /// Returns the original, unparsed full string path.
    pub fn full_path(&self) -> &'a str { self.full_path }
    /// Returns the primary name of the stat.
    pub fn name(&self) -> &'a str { self.name }
    /// Returns the optional part of the stat.
    pub fn part(&self) -> Option<&'a str> { self.part }
    /// Returns the optional numerical tag.
    pub fn tag(&self) -> Option<u32> { self.tag }
    /// Returns the optional target alias.
    pub fn target(&self) -> Option<&'a str> { self.target }
    /// Returns `true` if the stat path includes a target alias.
    pub fn has_target(&self) -> bool { self.target.is_some() }
    /// Returns `true` if the stat path includes a numerical tag.
    pub fn has_tags(&self) -> bool { self.tag.is_some() }

    /// Reconstructs the stat path string without the target alias, if one was present.
    /// Example: `"Damage.base.10@Player"` becomes `"Damage.base.10"`.
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