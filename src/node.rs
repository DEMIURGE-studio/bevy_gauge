use crate::context::AttributeContext;
use crate::modifier::{Modifier, TaggedModifier};
use crate::tags::TagMask;

/// How a attribute node's modifiers are reduced to produce a single value.
#[derive(Clone, Debug)]
pub enum ReduceFn {
    /// Sum all modifier values. Default for "added"/"flat" style attributes.
    Sum,
    /// Multiply all modifier values. Default for "more"/"less" style multipliers.
    /// The base is 1.0; each modifier is treated as `(1 + modifier_value)`.
    Product,
    /// User-defined reduction function.
    ///
    /// Receives one value per **expression modifier** plus one **accumulated
    /// value per flat tag-slot** (flat modifiers are fungible and stored
    /// accumulated per tag, not individually - see [`AttributeNode`]).
    ///
    /// Called with an empty slice when the node has no matching modifiers,
    /// so the function decides its own identity value (Sum's is 0, Product's
    /// is 1). Handle the empty case rather than indexing unconditionally.
    Custom(fn(&[f32]) -> f32),
}

impl Default for ReduceFn {
    fn default() -> Self {
        ReduceFn::Sum
    }
}

/// Accumulated flat modifiers sharing one exact [`TagMask`].
///
/// Flat modifiers are fungible within a tag: only the accumulated value and a
/// contributor count are stored. `value` means `Σv` under `Sum`/`Custom`
/// reduce and `Π(1+v)` under `Product` reduce. When `count` returns to zero
/// the slot is dropped, so accumulated float drift can never outlive the
/// modifiers that caused it.
///
/// A slot written by [`AttributeNode::set_flat_base`] is marked `base`. A
/// base slot is never dropped by removals: `set_base` establishes a value
/// that later modifier removals subtract from, not one they can wipe.
#[derive(Clone, Debug)]
struct FlatSlot {
    tag: TagMask,
    value: f32,
    count: u32,
    base: bool,
}

/// A attribute node - the fundamental unit of the attribute graph.
///
/// Stores modifiers in two forms:
/// - **Expression modifiers** are kept individually (each re-evaluates
///   dynamically against the context).
/// - **Flat modifiers** are accumulated per exact tag into [`FlatSlot`]s.
///   Adding `Flat(5.0)` is `+= 5.0` on the slot; removing it is `-= 5.0`.
///   There is no per-modifier identity for flats - they are fungible.
#[derive(Clone, Debug)]
pub struct AttributeNode {
    /// How modifiers are combined.
    pub reduce: ReduceFn,
    /// Expression modifiers, stored individually.
    exprs: Vec<TaggedModifier>,
    /// Flat modifiers, accumulated per exact tag.
    flats: Vec<FlatSlot>,
}

impl AttributeNode {
    /// Create a new node with the given reduce function and no modifiers.
    pub fn new(reduce: ReduceFn) -> Self {
        Self {
            reduce,
            exprs: Vec::new(),
            flats: Vec::new(),
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
        match modifier {
            Modifier::Flat(v) => self.accumulate_flat(tag, v),
            expr => self.exprs.push(TaggedModifier::new(expr, tag)),
        }
    }

    /// Remove a modifier from this node. Returns true if something was removed.
    ///
    /// - `Flat(v)` subtracts `v` from the **untagged** slot (flats are
    ///   fungible; there is no per-modifier lookup). Use
    ///   [`remove_tagged_modifier`](Self::remove_tagged_modifier) for tagged
    ///   flats.
    /// - `Expr` removes the first equal expression modifier, ignoring tags.
    pub fn remove_modifier(&mut self, modifier: &Modifier) -> bool {
        match modifier {
            Modifier::Flat(v) => self.un_accumulate_flat(TagMask::NONE, *v),
            expr => {
                if let Some(pos) = self.exprs.iter().position(|tm| &tm.modifier == expr) {
                    self.exprs.remove(pos);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Remove a modifier matching both value and tag.
    /// Returns true if something was removed.
    pub fn remove_tagged_modifier(&mut self, modifier: &Modifier, tag: TagMask) -> bool {
        match modifier {
            Modifier::Flat(v) => self.un_accumulate_flat(tag, *v),
            expr => {
                if let Some(pos) = self
                    .exprs
                    .iter()
                    .position(|tm| tm.tag == tag && &tm.modifier == expr)
                {
                    self.exprs.remove(pos);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// The "equivalent single flat" for a tag slot: the value `v` such that
    /// replacing the slot with one `Flat(v)` leaves the node unchanged.
    /// This is what `set_base` / `set_base_tagged` replace.
    pub fn flat_base(&self, tag: TagMask) -> f32 {
        match self.flats.iter().find(|s| s.tag == tag) {
            Some(slot) => match self.reduce {
                ReduceFn::Product => slot.value - 1.0,
                _ => slot.value,
            },
            None => 0.0,
        }
    }

    /// Replace a tag's flat slot with a base value of `value`.
    ///
    /// The slot becomes a **base slot**: its accumulated value is replaced,
    /// but the contributor count is preserved, so modifiers that were added
    /// before the call can still be removed (subtracting their value) and
    /// removal never drops the slot. Without this, `set_base` followed by
    /// removing one of the earlier contributors would drain the slot and
    /// wipe the base entirely.
    pub fn set_flat_base(&mut self, tag: TagMask, value: f32) {
        let slot_value = match self.reduce {
            ReduceFn::Product => 1.0 + value,
            _ => value,
        };
        if let Some(slot) = self.flats.iter_mut().find(|s| s.tag == tag) {
            slot.value = slot_value;
            slot.base = true;
        } else {
            self.flats.push(FlatSlot { tag, value: slot_value, count: 0, base: true });
        }
    }

    /// Iterate over the expression modifiers (used for dependency/source
    /// bookkeeping).
    pub(crate) fn expressions(&self) -> impl Iterator<Item = &crate::expr::Expr> {
        self.exprs.iter().filter_map(|tm| match &tm.modifier {
            Modifier::Expr(e) => Some(e),
            _ => None,
        })
    }

    fn accumulate_flat(&mut self, tag: TagMask, v: f32) {
        let is_product = matches!(self.reduce, ReduceFn::Product);
        if let Some(slot) = self.flats.iter_mut().find(|s| s.tag == tag) {
            if is_product {
                slot.value *= 1.0 + v;
            } else {
                slot.value += v;
            }
            slot.count += 1;
        } else {
            self.flats.push(FlatSlot {
                tag,
                value: if is_product { 1.0 + v } else { v },
                count: 1,
                base: false,
            });
        }
    }

    fn un_accumulate_flat(&mut self, tag: TagMask, v: f32) -> bool {
        let is_product = matches!(self.reduce, ReduceFn::Product);
        let Some(pos) = self
            .flats
            .iter()
            .position(|s| s.tag == tag && (s.count > 0 || s.base))
        else {
            return false;
        };
        let slot = &mut self.flats[pos];
        slot.count = slot.count.saturating_sub(1);
        if slot.count == 0 && !slot.base {
            // Exact reset: drift (and un-invertible x0 factors) can't outlive
            // the modifiers that caused them.
            self.flats.swap_remove(pos);
        } else if is_product {
            let divisor = 1.0 + v;
            // A modifier of exactly -1.0 zeroed the slot; it can't be divided
            // back out. The slot stays 0 until its count drains, then resets.
            if divisor != 0.0 {
                slot.value /= divisor;
            }
        } else {
            slot.value -= v;
        }
        true
    }

    /// Evaluate this node: evaluate **all** modifiers (ignoring tags), then reduce.
    pub fn evaluate(&self, context: &AttributeContext) -> f32 {
        self.combine(
            self.exprs.iter().map(|tm| tm.modifier.evaluate(context)),
            self.flats.iter().map(|s| s.value),
        )
    }

    /// Evaluate only modifiers whose tags match the given query, then reduce.
    ///
    /// A modifier matches if its tag is NONE (global) or its tag bits are a
    /// subset of `query`. See [`TagMask::matches_query`].
    pub fn evaluate_tagged(&self, context: &AttributeContext, query: TagMask) -> f32 {
        self.combine(
            self.exprs
                .iter()
                .filter(|tm| tm.tag.matches_query(query))
                .map(|tm| tm.modifier.evaluate(context)),
            self.flats
                .iter()
                .filter(|s| s.tag.matches_query(query))
                .map(|s| s.value),
        )
    }

    /// Reduce evaluated expression values and flat-slot values.
    ///
    /// Flat slots already carry the reduce-appropriate representation:
    /// `Σv` for Sum/Custom, `Π(1+v)` for Product.
    fn combine(
        &self,
        exprs: impl Iterator<Item = f32>,
        flats: impl Iterator<Item = f32>,
    ) -> f32 {
        match &self.reduce {
            ReduceFn::Sum => exprs.sum::<f32>() + flats.sum::<f32>(),
            ReduceFn::Product => {
                exprs.map(|v| 1.0 + v).product::<f32>() * flats.product::<f32>()
            }
            ReduceFn::Custom(f) => {
                let values: Vec<f32> = exprs.chain(flats).collect();
                f(&values)
            }
        }
    }
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
        // Custom reduce sees one accumulated value PER TAG SLOT, not one per
        // flat modifier: flats sharing a tag are fungible and pre-summed.
        node.add_tagged_modifier(Modifier::Flat(3.0), fire);
        node.add_tagged_modifier(Modifier::Flat(7.0), physical);
        node.add_modifier(Modifier::Flat(1.0));
        assert_eq!(node.evaluate(&ctx), 7.0);

        // Same-tag flats accumulate before the custom fn sees them.
        node.add_tagged_modifier(Modifier::Flat(2.0), fire); // fire slot: 5.0
        assert_eq!(node.evaluate(&ctx), 7.0);
        node.add_tagged_modifier(Modifier::Flat(4.0), fire); // fire slot: 9.0
        assert_eq!(node.evaluate(&ctx), 9.0);
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
