# bevy_gauge

An attribute system for [Bevy](https://bevyengine.org/) with expression-based modifiers, tagged filtering, cross-entity dependencies, and automatic propagation.

Built for games that need attribute systems beyond simple key-value stores — RPGs with derived attributes, ARPGs with PoE-style damage pipelines, or any game where attributes depend on other attributes (possibly on other entities) and need to stay in sync.

## Quick start

```rust
use bevy::prelude::*;
use bevy_attributes::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AttributesPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        attributes! {
            "Strength"  => 20.0,
            "Vitality"  => 15.0,
            "MaxHealth"  => "Vitality * 10.0 + 50.0",
        },
    ));
}
```

## Core concepts

### Attributes component

Every entity that participates in the attribute system gets an `Attributes` component.
It stores the attribute nodes and their cached evaluated values:

```rust
commands.spawn(Attributes::new());
```

### Reading attributes

Reading only requires `&Attributes` — no special system param needed:

```rust
fn print_health(query: Query<&Attributes>, interner: Res<Interner>) {
    for attrs in &query {
        let hp = attrs.get_by_name("MaxHealth", &interner);
        println!("Health: {hp}");
    }
}
```

### Writing attributes — `AttributesMut`

All writes go through the `AttributesMut` system parameter. This ensures
dependency edges are maintained and changes propagate automatically:

```rust
fn buff_strength(mut attributes: AttributesMut, entity: Entity) {
    attributes.add_modifier(entity, "Strength", 10.0);
}
```

`AttributesMut` is a `SystemParam` that bundles mutable access to the ECS query,
the interner, the dependency graph, and the tag resolver.

## Defining attributes

### Flat attributes

The simplest form — a attribute with a numeric value. Modifiers are summed:

```rust
attributes.flat_attribute(entity, "Armor", 50.0);
attributes.add_modifier(entity, "Armor", 25.0); // now 75
```

Or at spawn time using the `attributes!` macro:

```rust
commands.spawn((
    attributes! {
        "Strength"  => 50.0,
        "Dexterity" => 30.0,
    },
));
```

### Expression modifiers

Modifiers can be dynamic expressions that reference other attributes. When a
referenced attribute changes, dependents re-evaluate automatically:

```rust
// MaxHealth = Vitality * 10
attributes.add_expr_modifier(entity, "MaxHealth", "Vitality * 10.0")?;

// Dodge rating scales with dexterity
attributes.add_expr_modifier(entity, "DodgeChance", "Dexterity / 200.0")?;
```

The expression language supports:

| Feature | Syntax |
|---|---|
| Arithmetic | `+`, `-`, `*`, `/`, unary `-` |
| Parentheses | `(expr)` |
| Attribute references | `AttributeName`, `Attribute.Name` |
| Cross-entity refs | `AttributeName@Alias` |
| Tag queries | `Attribute{TAG\|TAG}` |
| Functions | `max(a, b)`, `min(a, b)`, `abs(x)`, `clamp(x, lo, hi)` |

Expressions compile to a compact bytecode VM (stack-based, no heap allocation at
eval time).

### Reduce functions

Each attribute node has a **reduce function** that controls how its modifiers combine:

| ReduceFn | Behavior | Use case |
|---|---|---|
| `Sum` (default) | `mod1 + mod2 + ...` | Flat / added values, % increases |
| `Product` | `(1+mod1) * (1+mod2) * ...` | Multiplicative "more" / "less" |
| `Custom(fn)` | User-defined `fn(&[f32]) -> f32` | Anything else |

```rust
attributes.add_modifier_with_reduce(
    entity, "DamageMultiplier", 0.2, ReduceFn::Product,
); // 1.2x
attributes.add_modifier_with_reduce(
    entity, "DamageMultiplier", 0.3, ReduceFn::Product,
); // 1.2 * 1.3 = 1.56x
```

### Complex attributes

A **complex attribute** is composed of named parts combined by an expression.
Each part is a separate attribute node that receives modifiers independently:

```rust
// PoE-style: base * (1 + increased) * more
attributes.complex_attribute(
    entity,
    "Damage",
    &[
        ("base",      ReduceFn::Sum),
        ("increased", ReduceFn::Sum),
        ("more",      ReduceFn::Product),
    ],
    "base * (1 + increased) * more",
)?;

// Add modifiers to individual parts
attributes.add_modifier(entity, "Damage.base", 100.0);
attributes.add_modifier(entity, "Damage.increased", 0.5);  // +50%
attributes.add_modifier(entity, "Damage.more", 0.2);        // 20% more → 1.2x

// Damage = 100 * 1.5 * 1.2 = 180
let total = attributes.evaluate(entity, "Damage");
```

Part names in the expression are short (`base`, `increased`). They are
automatically qualified to `Damage.base`, `Damage.increased`, etc.

## Tags and filtered evaluation

Tags let you attach metadata to modifiers and then query attributes with a filter.
This powers systems like PoE-style damage where the same attribute (`Damage.Added`)
has modifiers for different damage types and delivery methods.

### Defining tags

Tags are single bits in a `u64` bitmask. Register names in the `TagResolver`
so expressions can use `{TAG}` syntax:

```rust
const PHYSICAL: TagMask = TagMask::bit(0);
const FIRE:     TagMask = TagMask::bit(1);
const MELEE:    TagMask = TagMask::bit(2);

fn register_tags(mut resolver: ResMut<TagResolver>) {
    resolver.register("PHYSICAL", PHYSICAL);
    resolver.register("FIRE", FIRE);
    resolver.register("MELEE", MELEE);
}
```

### Tagged modifiers

Attach tags to modifiers. Untagged modifiers (`TagMask::NONE`) are **global** —
they participate in every query:

```rust
// 25 physical melee damage
attributes.add_modifier_tagged(entity, "Damage.Added", 25.0, PHYSICAL | MELEE);
// 10 fire melee damage
attributes.add_modifier_tagged(entity, "Damage.Added", 10.0, FIRE | MELEE);
// +5 generic melee damage (applies to ALL melee queries)
attributes.add_modifier_tagged(entity, "Damage.Added", 5.0, MELEE);
```

### Tag matching rule

A modifier participates in a query when **all** of its tag bits are present in
the query (the modifier's tags are a subset of the query):

```
Modifier [FIRE]         + Query [FIRE|MELEE] → matches (FIRE ⊆ FIRE|MELEE)
Modifier [FIRE|RANGED]  + Query [FIRE|MELEE] → no match (MELEE bit missing)
Modifier [NONE]         + Query [anything]   → always matches (global)
```

### Tagged evaluation

```rust
// Only modifiers whose tags ⊆ PHYSICAL|MELEE
let phys = attributes.evaluate_tagged(entity, "Damage.Added", PHYSICAL | MELEE);
```

### Tagged attributes (lazy materialization)

A **tagged attribute** combines parts with per-tag-combo expressions — and you
never have to enumerate combos up front. The system materializes expressions
lazily on first `evaluate_tagged` call:

```rust
attributes.tagged_attribute(
    entity,
    "Damage",
    &[("Added", ReduceFn::Sum), ("Increased", ReduceFn::Sum)],
    "Added * (1 + Increased)",
)?;

// Add tagged modifiers to the parts
attributes.add_modifier_tagged(entity, "Damage.Added", 25.0, PHYSICAL | MELEE);
attributes.add_modifier_tagged(entity, "Damage.Added", 10.0, FIRE | MELEE);

// Query any combo — expression auto-generates on first use
let phys = attributes.evaluate_tagged(entity, "Damage", PHYSICAL | MELEE);
let fire = attributes.evaluate_tagged(entity, "Damage", FIRE | MELEE);
```

When `evaluate_tagged(entity, "Damage", PHYSICAL | MELEE)` is called for the
first time, the system:

1. Decomposes `PHYSICAL | MELEE` into registered tag names
2. Qualifies the template: `"Damage.Added{PHYSICAL|MELEE} * (1 + Damage.Increased{PHYSICAL|MELEE})"`
3. Compiles, registers dependencies, and caches the result
4. Subsequent calls for the same combo are a no-op

## Dependencies between attributes

When a modifier is an expression referencing another attribute, the dependency graph
automatically tracks the relationship. Changes propagate recursively:

```rust
attributes.add_modifier(entity, "Vitality", 20.0);
attributes.add_expr_modifier(entity, "MaxHealth", "Vitality * 10.0")?;
attributes.add_expr_modifier(entity, "HealthRegen", "MaxHealth * 0.01")?;

// Changing Vitality propagates: Vitality → MaxHealth → HealthRegen
attributes.add_modifier(entity, "Vitality", 5.0); // all three update
```

The dependency graph is global (a `DependencyGraph` resource) and supports:
- **Local dependencies**: attribute A on the same entity depends on attribute B
- **Cross-entity dependencies**: attribute A on entity X depends on attribute B on entity Y
- **Tag query dependencies**: an expression reads a tag-filtered value

Cycles are detected at propagation time and short-circuited.

## Cross-entity dependencies

Attributes on one entity can reference attributes on another through **source aliases**.
This is how equipment, auras, buffs from other entities, etc. are modeled:

```rust
// The sword's damage scales with its wielder's Strength
attributes.add_expr_modifier_tagged(
    sword, "Damage.Increased", "Strength@Wielder / 200.0", PHYSICAL,
)?;

// Point the "Wielder" alias at the warrior entity
attributes.register_source(sword, "Wielder", warrior);

// Sword's Damage.Increased now reads warrior's Strength.
// Changing warrior's Strength auto-propagates to the sword.
```

### Swapping sources

Re-pointing an alias automatically rewires all dependency edges and
re-evaluates affected attributes:

```rust
// Hand the sword to the mage — one call, everything updates
attributes.register_source(sword, "Wielder", mage);
```

### Expression syntax

Cross-entity references use `@Alias` syntax: `"Strength@Wielder"`,
`"Intelligence@Parent"`, etc.

## Batch operations — `attributes!` and `mod_set!`

### `attributes!` — spawn-time initialization

Creates an `AttributeInitializer` component. When spawned alongside `Attributes`,
modifiers are automatically applied via an observer:

```rust
commands.spawn((
    Attributes::new(),
    attributes! {
        "Strength"     => 50.0,
        "Intelligence" => 10.0,
        "MaxHealth"    => "Strength * 2.0 + 100.0",
        "Damage.Added" [FIRE | MELEE] => 10.0,   // tagged
    },
));
```

Values can be `f32` literals (become flat modifiers) or string literals
(compiled as expression modifiers at apply time).

### `mod_set!` — runtime buffs/debuffs

Creates a `ModifierSet` that can be applied to any entity:

```rust
let fire_enchant = mod_set! {
    "Damage.Added" [FIRE | MELEE] => 20.0,
    "Damage.Increased" [FIRE]     => 0.15,
};
fire_enchant.apply(sword, &mut attributes);
```

## One-shot mutations — `InstantModifierSet`

`InstantModifierSet` applies attribute changes **once** without leaving persistent
modifiers on the attribute nodes. Used for ability effects, damage application,
and attributeus effect manipulation.

### `instant!` macro

```rust
let effects = instant! {
    "Scorch" += 1.0,                       // add to current value
    "Doom" += "-Doom@target",              // expression with role reference
    "ProjectileLife" -= 1.0,               // subtract from current value
    "Health" = "Strength@attacker * 0.5",  // overwrite value
};
```

Operators: `=` (set), `+=` (add), `-=` (subtract). Values can be `f32`
literals or expression strings.

### Role-based evaluation

Expressions can reference attributes on **role entities** via `@role` syntax.
Roles are temporary source aliases registered for the duration of evaluation:

```rust
let roles: &[(&str, Entity)] = &[
    ("attacker", attacker_entity),
    ("defender", defender_entity),
];

apply_instant(&effects, roles, defender_entity, &mut attributes);
```

The `evaluate_instant` / `apply_evaluated_instant` functions are also available
for two-phase evaluation if you need the concrete values before applying.

## Attribute requirements — `AttributeRequirements`

Boolean expressions over attributes that gate state-machine transitions, equipment
prerequisites, ability conditions, etc.

```rust
// As a component:
commands.spawn(requires! { "ProjectileLife <= 0" });

// Multiple requirements (all must be satisfied):
commands.spawn(requires! { "Strength >= 10", "Level >= 5" });
```

Check requirements in a system:

```rust
fn check_requirements(
    mut query: Query<&mut AttributeRequirements>,
    attrs_query: Query<&Attributes>,
    interner: Res<Interner>,
) {
    // ...
    if requirements.met(&attrs, &interner) {
        // all requirements satisfied
    }
}
```

Requirements are compiled lazily — source strings are stored at spawn time and
compiled to bytecode on the first `met()` call when the `Interner` is available.

## Derived components

### `#[derive(AttributeComponent)]` — the easy way

The `AttributeComponent` derive macro generates automatic `AttributeDerived`
and/or `WriteBack` implementations for your component:

```rust
#[derive(Component, Default, Debug, AttributeComponent)]
pub struct Life {
    #[read("Life")]
    pub max: f32,              // read from attribute "Life"
    #[write]
    pub current: f32,          // write back to "Life.current"
}
```

- `#[read]` / `#[read("path")]` reads from attributes (`AttributeDerived`)
- `#[write]` / `#[write("path")]` writes back to attributes (`WriteBack`)
- No argument auto-generates the path from `StructName.field_name`
- Explicit string paths are also supported
- Fields without an annotation are plain struct fields

Components with `#[derive(AttributeComponent)]` are **automatically registered** via
the `inventory` crate — no manual `app.register_*()` calls needed.

### `AttributeDerived` — manual implementation

A component whose fields are updated from attribute values whenever `Attributes`
changes. Implement the trait and register it:

```rust
#[derive(Component, Default)]
struct PlayerHealth {
    current: f32,
    max: f32,
}

impl AttributeDerived for PlayerHealth {
    fn should_update(&self, attrs: &Attributes, interner: &Interner) -> bool {
        let max = attrs.get_by_name("MaxHealth", interner);
        (self.max - max).abs() > f32::EPSILON
    }

    fn update_from_attributes(&mut self, attrs: &Attributes, interner: &Interner) {
        self.max = attrs.get_by_name("MaxHealth", interner);
    }
}
```

Register with the `inventory` auto-registration macro (runs at link time, no
manual app setup needed):

```rust
register_derived!(PlayerHealth);
```

Or register manually in your plugin if you prefer:

```rust
app.register_attribute_derived::<PlayerHealth>();
```

The update system runs in `PostUpdate` (in the `AttributeDerivedSet`) and only
processes entities whose `Attributes` changed since the last tick.

### `WriteBack` — write to attributes

A component whose fields are written back into the attribute system when
`Attributes` changes. Useful for input-driven attributes:

```rust
impl WriteBack for CombatInput {
    fn should_write_back(&self, attrs: &Attributes, interner: &Interner) -> bool {
        let current = attrs.get_by_name("AttackPower", interner);
        (self.attack_power - current).abs() > f32::EPSILON
    }

    fn write_back(&self, entity: Entity, attributes: &mut AttributesMut) {
        attributes.set(entity, "AttackPower", self.attack_power);
    }
}

register_write_back!(CombatInput);
```

Write-back systems run in `PostUpdate` before `AttributeDerived` systems, so
written values are available for derived reads in the same frame.

## API reference

### `AttributesMut` methods

| Method | Description |
|---|---|
| `add_modifier(entity, attribute, value)` | Add an untagged flat or expr modifier |
| `add_modifier_tagged(entity, attribute, value, tag)` | Add a tagged modifier |
| `add_expr_modifier(entity, attribute, expr_str)` | Add an expression modifier |
| `add_expr_modifier_tagged(entity, attribute, expr_str, tag)` | Add a tagged expression modifier |
| `add_modifier_with_reduce(entity, attribute, value, reduce)` | Add modifier with custom reduce fn |
| `remove_modifier(entity, attribute, modifier)` | Remove a modifier by value |
| `set(entity, attribute, value)` | Shorthand for adding a flat modifier |
| `set_base(entity, attribute, value)` | Replace all untagged flat modifiers with a single value |
| `get_attributes(entity)` | Read-only access to an entity's `Attributes` |
| `flat_attribute(entity, name, value)` | Create a simple flat attribute |
| `complex_attribute(entity, name, parts, expr)` | Create a multi-part attribute with expression |
| `tagged_attribute(entity, name, parts, expr)` | Create a lazily-materialized tagged attribute |
| `evaluate(entity, attribute)` | Force re-evaluate and return value |
| `evaluate_tagged(entity, attribute, tag)` | Evaluate with tag filter |
| `register_source(entity, alias, source)` | Link a cross-entity source alias |
| `unregister_source(entity, alias)` | Remove a source alias |

### Reading

| Method | On | Description |
|---|---|---|
| `get(id)` | `&Attributes` | Read cached value by `AttributeId` |
| `get_by_name(name, interner)` | `&Attributes` | Read by string name |
| `get_tagged(id, mask)` | `&Attributes` | Read cached tag-filtered value |
| `get_tagged_by_name(name, mask, interner)` | `&Attributes` | Read tag-filtered by name |

### Macros

| Macro | Description |
|---|---|
| `attributes! { ... }` | Spawn-time attribute initialization (creates `AttributeInitializer`) |
| `mod_set! { ... }` | Create a `ModifierSet` for runtime application |
| `instant! { ... }` | Create an `InstantModifierSet` for one-shot mutations |
| `requires! { ... }` | Create a `AttributeRequirements` component |
| `attribute_component! { ... }` | Generate a component with `AttributeDerived`/`WriteBack` impls |
| `register_derived!(T)` | Auto-register a `AttributeDerived` component via `inventory` |
| `register_write_back!(T)` | Auto-register a `WriteBack` component via `inventory` |

## Architecture notes

- **String interning**: attribute names are interned via `lasso` into `AttributeId` (`u32`).
  Lookups are integer comparisons, not string hashes.
- **Bytecode VM**: expressions compile to a stack-based bytecode with a 16-slot
  fixed stack. No heap allocation during evaluation.
- **No unsafe**: the crate contains no `unsafe` code.
- **Dependency propagation**: recursive DFS with cycle detection. Cross-entity
  source values are cached locally before evaluation.
- **Tag queries**: materialized as synthetic attribute nodes in the dependency graph.
  Once created, they propagate like any other attribute.

## Examples

Run the PoE-style tagged damage example:

```
cargo run --example rpg_combat
```

This demonstrates tagged attributes, cross-entity references, source swapping,
tag query specificity, and batch modifiers.

## License

MIT OR Apache-2.0
