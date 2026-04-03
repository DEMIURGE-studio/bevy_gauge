# bevy_gauge

A dependency-graph attribute system for [Bevy](https://bevy.org/).

Games with interconnected attributes - RPGs where strength affects health, ARPGs where damage is tagged and flows through a pipeline of base/increased/more modifiers, survival games where equipment stacks - need an attribute system that propagates changes automatically. Gauge gives you that so you can define relationships between attributes declaratively and let the engine handle the math.

## Why gauge

Without a framework, interconnected attributes become a maintenance nightmare. You end up hand-rolling update order, chasing stale values, and writing bespoke propagation logic every time you add a new stat. Gauge solves this once.

Gauge makes it easy to:

- **Define attributes as expressions, not update systems.** `"MaxHealth" => "Vitality * 10.0 + 50.0"` - change Vitality and MaxHealth updates automatically. No manual propagation, no ordering bugs.
- **Compose modifiers from multiple sources.** Equipment, buffs, passives, and auras all add modifiers to the same attribute. Equip a sword, apply an enchant, remove the enchant - gauge tracks what came from where.
- **Reference attributes across entities.** A weapon's damage scales with `"Strength@Wielder"`. Hand the weapon to someone else with one call and everything recalculates.
- **Filter by tags.** "+10 fire damage" applies to fire swords, fire bows, and fireballs. "+5 sword damage" applies to physical swords and fire swords. Query `FIRE | SWORD` and get exactly the modifiers that match. No manual bookkeeping.
- **Sync attributes with components.** Derive a macro on your `Health` component and its fields stay in sync with the attribute graph - reads, writes, and initialization all handled.

## Quick start

```rust
commands.spawn(attributes! {
    "Strength"  => 20.0,
    "MaxHealth"  => "Vitality * 10.0 + 50.0",
    "Damage.added" [DamageTags::FIRE] => 10.0,
});
```

Define tag hierarchies for filtered evaluation:

```rust
define_tags! {
    DamageTags,
    element { fire, cold, lightning },
    physical,
}
```

Sync component fields with the attribute graph automatically:

```rust
// reads the MaxHealth attribute from Attributes and updates the `max` 
// field of the component automatically whenever the underlying attribute 
// changes.
// writes the `current` field back to `Attributes` so it can be used in
// expressions elsewhere. Since no name is specified, it will write to
// `Health.current` - the name of the component, then the field.
// The current value init's from `MaxHealth` so you don't have to manually
// init the value yourself.
#[derive(Component, Default, AttributeComponent)]
pub struct Health {
    #[read("MaxHealth")] 
    pub max: f32,
    #[write]
    #[init_from("MaxHealth")]
    pub current: f32,
}
```

Equipment and buffs are modifier sets that apply and remove cleanly:

```rust
let enchant = mod_set! {
    "Damage.added" [DamageTags::FIRE] => 20.0,
    "Damage.increased" => 0.15,
};
enchant.apply(sword, &mut attributes);
enchant.remove(sword, &mut attributes);
```

## Version Table

| Bevy  | Gauge |
| ----- | ----- |
| 0.18  | 0.4   |

## License

MIT OR Apache-2.0
