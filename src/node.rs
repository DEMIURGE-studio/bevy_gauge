use crate::context::AttributeContext;
use crate::modifier::{Modifier, TaggedModifier};
use crate::tags::TagMask;

/// How a attribute node's modifiers are reduced to produce a single value.
#[derive(Clone, Debug)]
#[derive(Default)]
pub enum ReduceFn {
    /// Sum all modifier values. Default for "added"/"flat" style attributes.
    #[default]
    Sum,
    /// Multiply all modifier values. Default for "more"/"less" style multipliers.
    /// The base is 1.0; each modifier is treated as `(1 + modifier_value)`.
    Product,
    /// User-defined reduction function.
    ///
    /// Receives the evaluated value of every matching modifier, sorted by
    /// `f32::total_cmp` (see [`AttributeNode`]). The one exception is a tag
    /// with a base set by [`AttributeNode::set_flat_base`]: its flat modifiers
    /// arrive as a single value, the base.
    ///
    /// Called with an empty slice when the node has no matching modifiers,
    /// so the function decides its own identity value (Sum's is 0, Product's
    /// is 1). Handle the empty case rather than indexing unconditionally.
    Custom(fn(&[f32]) -> f32),
}

/// A base written by [`AttributeNode::set_flat_base`] for one exact tag.
///
/// The flat modifiers on the tag stay in the node, so the ones added before
/// the base can still be removed, and removing one subtracts it from the base
/// rather than wiping it. The tag's flats then contribute
/// `value + (current total - snapshot)` (`Sum`/`Custom`) or
/// `(1 + value) * (current / snapshot) - 1` (`Product`): exactly `value` until
/// a flat on the tag changes, and a function of the modifiers present, not of
/// the order they arrived in.
#[derive(Clone, Debug)]
struct FlatBase {
    tag: TagMask,
    /// The equivalent single flat requested by `set_flat_base`.
    value: f32,
    /// The tag's flat total ([`AttributeNode::flat_total`]) when the base was set.
    snapshot: f32,
}

/// A attribute node - the fundamental unit of the attribute graph.
///
/// Every modifier is stored individually, and evaluation sorts the evaluated
/// values by `f32::total_cmp` before reducing them. Floating-point addition is
/// not associative, so this is what makes a node's value depend only on which
/// modifiers it holds, never on the order they were added in: two machines
/// that applied the same modifiers in different orders agree bit for bit.
#[derive(Clone, Debug)]
pub struct AttributeNode {
    /// How modifiers are combined.
    pub reduce: ReduceFn,
    /// Expression and flat modifiers, in no meaningful order.
    modifiers: Vec<TaggedModifier>,
    /// Flat bases written by `set_flat_base`, at most one per tag.
    bases: Vec<FlatBase>,
}

impl AttributeNode {
    /// Create a new node with the given reduce function and no modifiers.
    pub fn new(reduce: ReduceFn) -> Self {
        Self {
            reduce,
            modifiers: Vec::new(),
            bases: Vec::new(),
        }
    }

    /// Create a new Sum-reducing node.
    pub fn sum() -> Self {
        Self::new(ReduceFn::Sum)
    }

    /// Create a new Product-reducing node.
    pub fn product() -> Self {
        Self::new(ReduceFn::Product)
    }

    /// Add a modifier to this node (untagged - applies to every tag query).
    pub fn add_modifier(&mut self, modifier: Modifier) {
        self.add_tagged_modifier(modifier, TagMask::NONE);
    }

    /// Add a tagged modifier to this node.
    pub fn add_tagged_modifier(&mut self, modifier: Modifier, tag: TagMask) {
        self.modifiers.push(TaggedModifier::new(modifier, tag));
    }

    /// Remove a modifier from this node. Returns true if something was removed.
    ///
    /// - `Flat(v)` removes an equal **untagged** flat. Use
    ///   [`remove_tagged_modifier`](Self::remove_tagged_modifier) for tagged
    ///   flats.
    /// - `Expr` removes an equal expression modifier, ignoring tags.
    pub fn remove_modifier(&mut self, modifier: &Modifier) -> bool {
        match modifier {
            Modifier::Flat(_) => self.remove_tagged_modifier(modifier, TagMask::NONE),
            expr => self.remove_where(|tm| &tm.modifier == expr),
        }
    }

    /// Remove a modifier matching both value and tag.
    /// Returns true if something was removed.
    pub fn remove_tagged_modifier(&mut self, modifier: &Modifier, tag: TagMask) -> bool {
        self.remove_where(|tm| tm.tag == tag && &tm.modifier == modifier)
    }

    /// Remove one modifier matching `pred`. Storage order is irrelevant to
    /// evaluation, so this swaps rather than shifts.
    fn remove_where(&mut self, pred: impl Fn(&TaggedModifier) -> bool) -> bool {
        match self.modifiers.iter().rposition(pred) {
            Some(pos) => {
                self.modifiers.swap_remove(pos);
                true
            }
            None => false,
        }
    }

    /// The "equivalent single flat" for a tag: the value `v` such that
    /// replacing the tag's flat modifiers with one `Flat(v)` leaves the node
    /// unchanged. This is what `set_base` / `set_base_tagged` replace.
    pub fn flat_base(&self, tag: TagMask) -> f32 {
        let total = self.flat_total(tag);
        let is_product = matches!(self.reduce, ReduceFn::Product);
        match self.bases.iter().find(|b| b.tag == tag) {
            Some(base) if total == base.snapshot => base.value,
            Some(base) if is_product => (1.0 + base.value) * (total / base.snapshot) - 1.0,
            Some(base) => base.value + (total - base.snapshot),
            None if is_product => total - 1.0,
            None => total,
        }
    }

    /// Set a tag's flat base to `value`.
    ///
    /// The tag's flats then read as `value`, but the modifiers that were added
    /// before the call stay in the node: removing one later subtracts it from
    /// the base instead of wiping the base, and new flats stack on top of it.
    pub fn set_flat_base(&mut self, tag: TagMask, value: f32) {
        let mut snapshot = self.flat_total(tag);
        if matches!(self.reduce, ReduceFn::Product) && snapshot == 0.0 {
            // A `Flat(-1.0)` zeroed the product; it can't be divided back out
            // later, so the base replaces the tag's flats outright.
            self.modifiers
                .retain(|tm| !(tm.tag == tag && matches!(tm.modifier, Modifier::Flat(_))));
            snapshot = 1.0;
        }
        let base = FlatBase { tag, value, snapshot };
        match self.bases.iter_mut().find(|b| b.tag == tag) {
            Some(existing) => *existing = base,
            None => self.bases.push(base),
        }
    }

    /// The flat modifiers on exactly `tag`, reduced in canonical order, without
    /// any base: `Σv` under `Sum`/`Custom`, `Π(1+v)` under `Product`.
    fn flat_total(&self, tag: TagMask) -> f32 {
        let flats = self.modifiers.iter().filter_map(|tm| match tm.modifier {
            Modifier::Flat(v) if tm.tag == tag => Some(v),
            _ => None,
        });
        match self.reduce {
            ReduceFn::Product => {
                with_canonical_order(flats, |values| values.iter().map(|v| 1.0 + v).product())
            }
            _ => with_canonical_order(flats, |values| values.iter().sum()),
        }
    }

    fn has_base(&self, tag: TagMask) -> bool {
        self.bases.iter().any(|b| b.tag == tag)
    }

    /// Iterate over the expression modifiers (used for dependency/source
    /// bookkeeping).
    pub(crate) fn expressions(&self) -> impl Iterator<Item = &crate::expr::Expr> {
        self.modifiers.iter().filter_map(|tm| match &tm.modifier {
            Modifier::Expr(e) => Some(e),
            _ => None,
        })
    }

    /// Evaluate this node: evaluate **all** modifiers (ignoring tags), then reduce.
    pub fn evaluate(&self, context: &AttributeContext) -> f32 {
        self.evaluate_where(context, |_| true)
    }

    /// Evaluate only modifiers whose tags match the given query, then reduce.
    ///
    /// A modifier matches if its tag is NONE (global) or its tag bits are a
    /// subset of `query`. See [`TagMask::matches_query`].
    pub fn evaluate_tagged(&self, context: &AttributeContext, query: TagMask) -> f32 {
        self.evaluate_where(context, |tag| tag.matches_query(query))
    }

    /// Reduce every modifier whose tag passes `include`. A tag with a base
    /// contributes its flats as one value, [`flat_base`](Self::flat_base).
    fn evaluate_where(&self, context: &AttributeContext, include: impl Fn(TagMask) -> bool) -> f32 {
        let modifiers = self
            .modifiers
            .iter()
            .filter(|tm| include(tm.tag))
            .filter(|tm| !(matches!(tm.modifier, Modifier::Flat(_)) && self.has_base(tm.tag)))
            .map(|tm| tm.modifier.evaluate(context));
        let bases = self
            .bases
            .iter()
            .filter(|b| include(b.tag))
            .map(|b| self.flat_base(b.tag));
        self.reduce_iter(modifiers.chain(bases))
    }

    /// Reduce an iterator of evaluated modifier values using this node's reduce function.
    ///
    /// The values are put in a canonical order (`f32::total_cmp`) before they are folded, so the
    /// result depends only on which modifiers the node holds, never on the order they were added
    /// in: floating-point addition is not associative, and `0.1 + 1e8 - 1e8` and
    /// `-1e8 + 1e8 + 0.1` differ. That is what lets two machines that applied the same modifiers in
    /// different orders agree bit for bit. Up to [`INLINE_VALUES`] values are sorted on the stack;
    /// more spill to a `Vec`. Custom reducers receive the values in the same order.
    fn reduce_iter(&self, iter: impl Iterator<Item = f32>) -> f32 {
        with_canonical_order(iter, |values| match &self.reduce {
            ReduceFn::Sum => values.iter().sum(),
            ReduceFn::Product => values.iter().map(|v| 1.0 + v).product(),
            ReduceFn::Custom(f) => f(values),
        })
    }
}

/// How many evaluated values [`AttributeNode::reduce_iter`] orders on the stack before spilling.
const INLINE_VALUES: usize = 16;

/// Collects `iter`, sorts it by `f32::total_cmp`, and hands the result to `reduce`.
///
/// `total_cmp` is a total order on the bit patterns, so equal values are bit-identical and an
/// unstable sort is as canonical as a stable one.
fn with_canonical_order<R>(iter: impl Iterator<Item = f32>, reduce: impl FnOnce(&[f32]) -> R) -> R {
    let mut inline = [0.0_f32; INLINE_VALUES];
    let mut len = 0;
    let mut spilled: Vec<f32> = Vec::new();
    for value in iter {
        if spilled.is_empty() && len < INLINE_VALUES {
            inline[len] = value;
            len += 1;
        } else {
            if spilled.is_empty() {
                spilled.extend_from_slice(&inline[..len]);
            }
            spilled.push(value);
        }
    }
    let values: &mut [f32] = if spilled.is_empty() { &mut inline[..len] } else { &mut spilled };
    values.sort_unstable_by(f32::total_cmp);
    reduce(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_reduce_sees_empty_input() {
        fn floor_ten(values: &[f32]) -> f32 {
            values.iter().copied().fold(10.0, f32::max)
        }
        let ctx = AttributeContext::default();
        let mut node = AttributeNode::new(ReduceFn::Custom(floor_ten));
        assert_eq!(node.evaluate(&ctx), 10.0, "identity comes from the function");
        node.add_modifier(Modifier::Flat(25.0));
        assert_eq!(node.evaluate(&ctx), 25.0);
        assert_eq!(node.evaluate_tagged(&ctx, TagMask::bit(3)), 25.0, "global flat matches");
    }

    #[test]
    fn sum_node() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::sum();
        node.add_modifier(Modifier::Flat(10.0));
        node.add_modifier(Modifier::Flat(5.0));
        assert_eq!(node.evaluate(&ctx), 15.0);
    }

    #[test]
    fn product_node() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::product();
        node.add_modifier(Modifier::Flat(0.5)); // 1.5x
        node.add_modifier(Modifier::Flat(0.3)); // 1.3x
        let result = node.evaluate(&ctx);
        // 1.5 * 1.3 = 1.95
        assert!((result - 1.95).abs() < 0.001);
    }

    #[test]
    fn empty_sum_is_zero() {
        let ctx = AttributeContext::new();
        let node = AttributeNode::sum();
        assert_eq!(node.evaluate(&ctx), 0.0);
    }

    #[test]
    fn empty_product_is_one() {
        let ctx = AttributeContext::new();
        let node = AttributeNode::product();
        assert_eq!(node.evaluate(&ctx), 1.0);
    }

    #[test]
    fn remove_modifier() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::sum();
        node.add_modifier(Modifier::Flat(10.0));
        node.add_modifier(Modifier::Flat(5.0));
        assert!(node.remove_modifier(&Modifier::Flat(10.0)));
        assert_eq!(node.evaluate(&ctx), 5.0);
    }

    #[test]
    fn remove_flat_from_empty_slot_is_noop() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::sum();
        assert!(!node.remove_modifier(&Modifier::Flat(10.0)));
        node.add_modifier(Modifier::Flat(5.0));
        assert!(node.remove_modifier(&Modifier::Flat(5.0)));
        // Slot drained - removal reports false and the value stays exact.
        assert!(!node.remove_modifier(&Modifier::Flat(5.0)));
        assert_eq!(node.evaluate(&ctx), 0.0);
    }

    #[test]
    fn drained_slot_resets_exactly() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::sum();
        node.add_modifier(Modifier::Flat(1e8));
        node.add_modifier(Modifier::Flat(0.25));
        // 0.25 is absorbed by 1e8 in f32 - but draining the slot resets it.
        node.remove_modifier(&Modifier::Flat(1e8));
        node.remove_modifier(&Modifier::Flat(0.25));
        assert_eq!(node.evaluate(&ctx), 0.0);
    }

    #[test]
    fn product_remove_divides_back_out() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::product();
        node.add_modifier(Modifier::Flat(0.5)); // 1.5x
        node.add_modifier(Modifier::Flat(0.3)); // 1.3x
        assert!(node.remove_modifier(&Modifier::Flat(0.3)));
        let result = node.evaluate(&ctx);
        assert!((result - 1.5).abs() < 1e-5);
    }

    #[test]
    fn custom_reduce() {
        let ctx = AttributeContext::new();
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);

        let mut node = AttributeNode::new(ReduceFn::Custom(|vals| {
            vals.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        }));
        // Custom reduce sees one value per modifier.
        node.add_tagged_modifier(Modifier::Flat(3.0), fire);
        node.add_tagged_modifier(Modifier::Flat(7.0), physical);
        node.add_modifier(Modifier::Flat(1.0));
        assert_eq!(node.evaluate(&ctx), 7.0);

        // Same-tag flats are not pre-summed: max(3, 2, 4, 7, 1) is still 7.
        node.add_tagged_modifier(Modifier::Flat(2.0), fire);
        node.add_tagged_modifier(Modifier::Flat(4.0), fire);
        assert_eq!(node.evaluate(&ctx), 7.0);

        // A base collapses its tag's flats into one value: fire reads 9.
        node.set_flat_base(fire, 9.0);
        assert_eq!(node.evaluate(&ctx), 9.0);
    }

    #[test]
    fn base_only_absorbs_its_own_tag() {
        let ctx = AttributeContext::new();
        let fire = TagMask::bit(0);
        let mut node = AttributeNode::sum();
        node.add_modifier(Modifier::Flat(5.0));
        node.add_tagged_modifier(Modifier::Flat(10.0), fire);
        node.set_flat_base(fire, 3.0);
        assert_eq!(node.evaluate(&ctx), 8.0);
        assert_eq!(node.evaluate_tagged(&ctx, fire), 8.0);
        assert_eq!(node.evaluate_tagged(&ctx, TagMask::bit(1)), 5.0);
    }

    #[test]
    fn a_based_sum_is_the_same_in_any_order() {
        // Same flats, same base, added in opposite orders: identical bits.
        let ctx = AttributeContext::new();
        let values = [0.1_f32, 1.0e8, -1.0e8, 3.7];
        let mut forward = AttributeNode::sum();
        let mut backward = AttributeNode::sum();
        for v in values {
            forward.add_modifier(Modifier::Flat(v));
        }
        for v in values.iter().rev() {
            backward.add_modifier(Modifier::Flat(*v));
        }
        forward.set_flat_base(TagMask::NONE, 50.0);
        backward.set_flat_base(TagMask::NONE, 50.0);
        forward.remove_modifier(&Modifier::Flat(1.0e8));
        backward.remove_modifier(&Modifier::Flat(1.0e8));
        assert_eq!(forward.evaluate(&ctx).to_bits(), backward.evaluate(&ctx).to_bits());
    }

    /// Every permutation of `values` added to a fresh node of `reduce`, evaluated, as bits.
    fn evaluations(reduce: fn() -> AttributeNode, values: &[f32]) -> Vec<u32> {
        fn permutations(values: &[f32]) -> Vec<Vec<f32>> {
            if values.len() <= 1 {
                return vec![values.to_vec()];
            }
            let mut out = Vec::new();
            for i in 0..values.len() {
                let mut rest = values.to_vec();
                let first = rest.remove(i);
                for mut tail in permutations(&rest) {
                    tail.insert(0, first);
                    out.push(tail);
                }
            }
            out
        }
        let ctx = AttributeContext::new();
        permutations(values)
            .into_iter()
            .map(|order| {
                let mut node = reduce();
                for value in order {
                    node.add_modifier(Modifier::Flat(value));
                }
                node.evaluate(&ctx).to_bits()
            })
            .collect()
    }

    #[test]
    fn a_sum_is_the_same_in_any_order() {
        // In arrival order, 0.1 + 1e8 - 1e8 is 0 while -1e8 + 1e8 + 0.1 is 0.1.
        let bits = evaluations(AttributeNode::sum, &[0.1, 1.0e8, -1.0e8, 3.7]);
        assert!(bits.iter().all(|b| *b == bits[0]), "{bits:?}");
    }

    #[test]
    fn a_product_is_the_same_in_any_order() {
        let bits = evaluations(AttributeNode::product, &[0.1, 1.0e7, -0.999_999, 0.3]);
        assert!(bits.iter().all(|b| *b == bits[0]), "{bits:?}");
    }

    #[test]
    fn more_values_than_fit_inline_are_still_ordered() {
        let ctx = AttributeContext::new();
        // 0.1 is lost to 1e8 forwards and kept backwards; the zeros push it past the inline buffer.
        let mut values = vec![0.1_f32, 1.0e8, -1.0e8];
        values.resize(INLINE_VALUES + 5, 0.0);
        let mut forward = AttributeNode::sum();
        let mut backward = AttributeNode::sum();
        for value in &values {
            forward.add_modifier(Modifier::Flat(*value));
        }
        for value in values.iter().rev() {
            backward.add_modifier(Modifier::Flat(*value));
        }
        assert_eq!(
            forward.evaluate(&ctx).to_bits(),
            backward.evaluate(&ctx).to_bits()
        );
    }

    #[test]
    fn a_custom_reducer_sees_the_values_in_order() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::new(ReduceFn::Custom(|vals| vals[0]));
        node.add_modifier(Modifier::Flat(3.0));
        node.add_modifier(Modifier::Flat(-7.0));
        node.add_modifier(Modifier::Flat(1.0));
        assert_eq!(node.evaluate(&ctx), -7.0);
    }

    // --- Tagged modifier tests ---

    #[test]
    fn tagged_evaluate_filters_by_query() {
        let ctx = AttributeContext::new();
        let fire = TagMask::bit(0);
        let physical = TagMask::bit(1);
        let melee = TagMask::bit(2);

        let mut node = AttributeNode::sum();
        node.add_tagged_modifier(Modifier::Flat(25.0), physical | melee);
        node.add_tagged_modifier(Modifier::Flat(10.0), fire | melee);
        node.add_modifier(Modifier::Flat(5.0)); // global

        // Unfiltered: all modifiers
        assert_eq!(node.evaluate(&ctx), 40.0);

        // PHYSICAL|MELEE: physical+melee modifier (25) + global (5) = 30
        assert_eq!(node.evaluate_tagged(&ctx, physical | melee), 30.0);

        // FIRE|MELEE: fire+melee modifier (10) + global (5) = 15
        assert_eq!(node.evaluate_tagged(&ctx, fire | melee), 15.0);

        // MELEE only: global (5) only - neither tagged modifier is a subset
        assert_eq!(node.evaluate_tagged(&ctx, melee), 5.0);

        // FIRE|PHYSICAL|MELEE: all three match = 25 + 10 + 5 = 40
        assert_eq!(
            node.evaluate_tagged(&ctx, fire | physical | melee),
            40.0
        );
    }

    #[test]
    fn remove_tagged_modifier_matches_tag() {
        let ctx = AttributeContext::new();
        let fire = TagMask::bit(0);

        let mut node = AttributeNode::sum();
        node.add_tagged_modifier(Modifier::Flat(10.0), fire);
        node.add_modifier(Modifier::Flat(10.0)); // same value, NONE tag

        // Remove only the FIRE-tagged one
        assert!(node.remove_tagged_modifier(&Modifier::Flat(10.0), fire));
        assert_eq!(node.evaluate(&ctx), 10.0); // global remains
    }

    #[test]
    fn flat_base_and_set_flat_base() {
        let ctx = AttributeContext::new();
        let mut node = AttributeNode::sum();
        node.add_modifier(Modifier::Flat(10.0));
        node.add_modifier(Modifier::Flat(5.0));
        assert_eq!(node.flat_base(TagMask::NONE), 15.0);

        node.set_flat_base(TagMask::NONE, 42.0);
        assert_eq!(node.evaluate(&ctx), 42.0);
        assert_eq!(node.flat_base(TagMask::NONE), 42.0);

        // Product: flat_base is the equivalent single flat.
        let mut prod = AttributeNode::product();
        prod.add_modifier(Modifier::Flat(0.5));
        prod.add_modifier(Modifier::Flat(0.3));
        assert!((prod.flat_base(TagMask::NONE) - 0.95).abs() < 1e-5); // 1.95 - 1
        prod.set_flat_base(TagMask::NONE, 0.5);
        assert!((prod.evaluate(&ctx) - 1.5).abs() < 1e-5);
    }
}
