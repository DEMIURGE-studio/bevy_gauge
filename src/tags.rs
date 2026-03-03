use std::collections::HashMap;

use bevy::prelude::*;

/// A bitmask representing a set of tags on a modifier or a tag query.
///
/// Tags enable filtered attribute evaluation — e.g., "fire sword damage" uses
/// only modifiers that apply to fire and/or sword damage.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TagMask(pub u64);

impl TagMask {
    /// The empty tag mask (matches everything in queries, applies to everything as a modifier).
    pub const NONE: TagMask = TagMask(0);

    /// Create a tag mask from a raw u64 value.
    pub const fn new(bits: u64) -> Self {
        Self(bits)
    }

    /// Create a tag mask with a single bit set.
    pub const fn bit(index: u32) -> Self {
        Self(1u64 << index)
    }

    /// Combine two tag masks (bitwise OR).
    pub const fn union(self, other: TagMask) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if this mask satisfies a query mask.
    ///
    /// A modifier with `self` tags satisfies query `q` when `(self & q) == q`.
    /// This means the modifier has at least all the bits the query asks for.
    ///
    /// Special case: query of NONE (0) is satisfied by everything.
    pub const fn satisfies(self, query: TagMask) -> bool {
        query.0 == 0 || (self.0 & query.0) == query.0
    }

    /// Check whether a modifier with this tag should participate in a given query.
    ///
    /// A modifier with tag `self` matches query `q` when:
    /// - `self` is NONE (the modifier is global — it applies to every query), OR
    /// - All of `self`'s tag bits are present in `q` (the modifier's tags are a
    ///   subset of the query).
    ///
    /// # Examples
    ///
    /// ```
    /// # use bevy_attributes::prelude::TagMask;
    /// let fire = TagMask::bit(0);
    /// let physical = TagMask::bit(1);
    /// let melee = TagMask::bit(2);
    ///
    /// // Global modifier (NONE) matches any query
    /// assert!(TagMask::NONE.matches_query(fire));
    ///
    /// // FIRE modifier matches a FIRE query
    /// assert!(fire.matches_query(fire));
    ///
    /// // FIRE modifier matches a FIRE|MELEE query (fire ⊆ fire|melee)
    /// assert!(fire.matches_query(fire | melee));
    ///
    /// // FIRE modifier does NOT match a PHYSICAL query
    /// assert!(!fire.matches_query(physical));
    ///
    /// // FIRE|MELEE modifier does NOT match a FIRE-only query (melee bit missing)
    /// assert!(!(fire | melee).matches_query(fire));
    /// ```
    pub const fn matches_query(self, query: TagMask) -> bool {
        self.0 == 0 || (self.0 & query.0) == self.0
    }

    /// Check if this mask is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for TagMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for TagMask {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// ---------------------------------------------------------------------------
// TagResolver — ECS resource mapping tag name strings to TagMask values
// ---------------------------------------------------------------------------

/// ECS resource that maps tag name strings to [`TagMask`] values.
///
/// This replaces the need for a global static configuration. Tag names are
/// registered at app startup (manually or via a future `define_tags!` macro)
/// and used at expression compile time to resolve `{FIRE|SPELL}` syntax.
#[derive(Resource, Default, Debug)]
pub struct TagResolver {
    tags: HashMap<String, TagMask>,
    /// Reverse mapping: bit position → registered tag name.
    /// Only populated for single-bit masks registered via [`register`](Self::register).
    reverse_tags: HashMap<u32, String>,
}

impl TagResolver {
    /// Create a new empty resolver.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tag name → mask mapping.
    ///
    /// If the name was already registered, the old mapping is overwritten.
    /// Single-bit masks also populate a reverse lookup (bit position → name)
    /// used by [`decompose`](Self::decompose).
    pub fn register(&mut self, name: &str, mask: TagMask) {
        let upper = name.to_uppercase();
        self.tags.insert(upper.clone(), mask);
        // Record reverse mapping for single-bit masks
        if mask.0.count_ones() == 1 {
            self.reverse_tags.insert(mask.0.trailing_zeros(), upper);
        }
    }

    /// Resolve a tag name to its mask. Case-insensitive.
    /// Returns `None` if the tag name hasn't been registered.
    pub fn resolve(&self, name: &str) -> Option<TagMask> {
        self.tags.get(&name.to_uppercase()).copied()
    }

    /// Resolve multiple tag names and OR them together.
    /// Unknown tag names are silently ignored (contribute 0 bits).
    pub fn resolve_set(&self, names: &[&str]) -> TagMask {
        names
            .iter()
            .filter_map(|name| self.resolve(name))
            .fold(TagMask::NONE, |acc, m| acc | m)
    }

    /// Decompose a [`TagMask`] into the registered names for each set bit.
    ///
    /// Returns `None` if any set bit in the mask doesn't have a registered
    /// single-bit name. Returns an empty `Vec` for [`TagMask::NONE`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// resolver.register("FIRE", TagMask::bit(0));
    /// resolver.register("MELEE", TagMask::bit(2));
    ///
    /// let names = resolver.decompose(TagMask::bit(0) | TagMask::bit(2));
    /// assert_eq!(names, Some(vec!["FIRE", "MELEE"]));
    /// ```
    pub fn decompose(&self, mask: TagMask) -> Option<Vec<&str>> {
        if mask.is_empty() {
            return Some(Vec::new());
        }
        let mut names = Vec::new();
        let mut bits = mask.0;
        while bits != 0 {
            let bit_pos = bits.trailing_zeros();
            let name = self.reverse_tags.get(&bit_pos)?;
            names.push(name.as_str());
            bits &= bits - 1; // clear lowest set bit
        }
        Some(names)
    }

    /// Build a `{TAG1|TAG2}` expression-syntax suffix string for the given mask.
    ///
    /// Returns `None` if the mask can't be decomposed (see [`decompose`](Self::decompose)).
    /// Returns `Some("")` for [`TagMask::NONE`].
    pub fn tag_suffix(&self, mask: TagMask) -> Option<String> {
        let names = self.decompose(mask)?;
        if names.is_empty() {
            Some(String::new())
        } else {
            Some(format!("{{{}}}", names.join("|")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything() {
        let fire = TagMask::bit(0);
        let sword = TagMask::bit(4);
        let both = fire | sword;

        assert!(fire.satisfies(TagMask::NONE));
        assert!(sword.satisfies(TagMask::NONE));
        assert!(both.satisfies(TagMask::NONE));
        assert!(TagMask::NONE.satisfies(TagMask::NONE));
    }

    #[test]
    fn exact_match() {
        let fire = TagMask::bit(0);
        assert!(fire.satisfies(fire));
    }

    #[test]
    fn superset_satisfies_subset() {
        let fire = TagMask::bit(0);
        let sword = TagMask::bit(4);
        let fire_sword = fire | sword;

        // fire|sword modifier satisfies a query for just fire
        assert!(fire_sword.satisfies(fire));
        // fire|sword modifier satisfies a query for just sword
        assert!(fire_sword.satisfies(sword));
        // fire|sword modifier satisfies a query for fire|sword
        assert!(fire_sword.satisfies(fire_sword));
    }

    #[test]
    fn subset_does_not_satisfy_superset() {
        let fire = TagMask::bit(0);
        let sword = TagMask::bit(4);
        let fire_sword = fire | sword;

        // A fire-only modifier does NOT satisfy a query for fire|sword
        assert!(!fire.satisfies(fire_sword));
    }

    #[test]
    fn disjoint_does_not_satisfy() {
        let fire = TagMask::bit(0);
        let cold = TagMask::bit(1);
        assert!(!fire.satisfies(cold));
    }

    // --- matches_query tests ---

    #[test]
    fn global_modifier_matches_any_query() {
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);
        assert!(TagMask::NONE.matches_query(fire));
        assert!(TagMask::NONE.matches_query(physical));
        assert!(TagMask::NONE.matches_query(fire | physical));
        assert!(TagMask::NONE.matches_query(TagMask::NONE));
    }

    #[test]
    fn exact_tag_matches_query() {
        let fire = TagMask::bit(0);
        assert!(fire.matches_query(fire));
    }

    #[test]
    fn subset_modifier_matches_superset_query() {
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        // FIRE modifier matches FIRE|MELEE query
        assert!(fire.matches_query(fire | melee));
    }

    #[test]
    fn superset_modifier_does_not_match_subset_query() {
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        // FIRE|MELEE modifier does NOT match a FIRE-only query
        assert!(!(fire | melee).matches_query(fire));
    }

    #[test]
    fn disjoint_modifier_does_not_match() {
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);
        assert!(!fire.matches_query(physical));
    }

    // --- TagResolver tests ---

    #[test]
    fn resolver_register_and_resolve() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        resolver.register("FIRE", fire);
        assert_eq!(resolver.resolve("FIRE"), Some(fire));
        assert_eq!(resolver.resolve("fire"), Some(fire)); // case insensitive
    }

    #[test]
    fn resolver_unknown_tag_returns_none() {
        let resolver = TagResolver::new();
        assert_eq!(resolver.resolve("UNKNOWN"), None);
    }

    #[test]
    fn resolver_resolve_set() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        resolver.register("FIRE", fire);
        resolver.register("MELEE", melee);
        assert_eq!(resolver.resolve_set(&["FIRE", "MELEE"]), fire | melee);
    }

    #[test]
    fn resolver_resolve_set_ignores_unknown() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        resolver.register("FIRE", fire);
        assert_eq!(resolver.resolve_set(&["FIRE", "NOPE"]), fire);
    }

    // --- decompose tests ---

    #[test]
    fn decompose_empty_mask() {
        let resolver = TagResolver::new();
        assert_eq!(resolver.decompose(TagMask::NONE), Some(vec![]));
    }

    #[test]
    fn decompose_single_bit() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        assert_eq!(resolver.decompose(TagMask::bit(0)), Some(vec!["FIRE"]));
    }

    #[test]
    fn decompose_multi_bit() {
        let mut resolver = TagResolver::new();
        let fire = TagMask::bit(0);
        let melee = TagMask::bit(2);
        resolver.register("FIRE", fire);
        resolver.register("MELEE", melee);

        let names = resolver.decompose(fire | melee).unwrap();
        assert!(names.contains(&"FIRE"));
        assert!(names.contains(&"MELEE"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn decompose_unregistered_bit_returns_none() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        // bit 1 is not registered
        assert_eq!(resolver.decompose(TagMask::bit(0) | TagMask::bit(1)), None);
    }

    #[test]
    fn tag_suffix_string() {
        let mut resolver = TagResolver::new();
        resolver.register("FIRE", TagMask::bit(0));
        resolver.register("MELEE", TagMask::bit(2));

        // Single tag
        let s = resolver.tag_suffix(TagMask::bit(0)).unwrap();
        assert_eq!(s, "{FIRE}");

        // Multi-tag
        let s = resolver.tag_suffix(TagMask::bit(0) | TagMask::bit(2)).unwrap();
        assert!(s.starts_with('{') && s.ends_with('}'));
        assert!(s.contains("FIRE") && s.contains("MELEE"));

        // Empty
        assert_eq!(resolver.tag_suffix(TagMask::NONE), Some(String::new()));

        // Unresolvable
        assert_eq!(resolver.tag_suffix(TagMask::bit(5)), None);
    }
}
