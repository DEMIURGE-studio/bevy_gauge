/// Create a [`ModifierSet`](crate::modifier_set::ModifierSet) from a set
/// of attribute definitions.
///
/// # Syntax
///
/// ```ignore
/// mod_set! {
///     "AttributeName" => value,                         // untagged modifier
///     "AttributeName" [TAG_EXPR] => value,              // tagged modifier
///     attr::SOME_NAME => value,                          // name from a &str const/path
///     @build ComplexAttribute::new(...),                 // attribute builder
/// }
/// ```
///
/// - **Attribute name** can be a string literal (`"Damage.base"`) or any path
///   that resolves to a `&str`, such as a shared constant (`attr::DAMAGE`), so
///   names can be defined once and renamed safely.
/// - **`value`** can be an `f32` literal (becomes a flat modifier) or a
///   `&str` / string literal (becomes an expression modifier compiled at
///   apply time).
/// - **`TAG_EXPR`** is any Rust expression that evaluates to a [`TagMask`].
///   Typically `FIRE | MELEE` or `DamageTags::PHYSICAL`.
/// - **`@build`** adds an [`AttributeBuilder`](crate::modifier_set::AttributeBuilder)
///   that runs before modifier entries during apply.
///
/// # Example
///
/// ```ignore
/// let set = mod_set! {
///     "Damage.base" => 50.0,
///     "Damage.Added" [FIRE | MELEE] => 10.0,
///     @build ComplexAttribute::new("Damage",
///         &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
///         "base * (1 + increased)",
///     ),
/// };
/// set.apply_all(entity, &mut attributes);
/// ```
#[macro_export]
macro_rules! mod_set {
    // ── @munch arms (listed before the entry point to avoid shadowing) ──

    // Terminal: nothing left
    (@munch $set:ident,) => {};

    // Complex attribute shorthand: @complex "name" => [parts] => "expr"
    (@munch $set:ident, @complex $name:literal => [ $( ($part:literal, $reduce:expr) ),* $(,)? ] => $expr:literal , $($rest:tt)*) => {
        $set.add_builder($crate::modifier_set::ComplexAttribute::new(
            $name, &[ $( ($part, $reduce) ),* ], $expr,
        ));
        $crate::mod_set!(@munch $set, $($rest)*);
    };
    (@munch $set:ident, @complex $name:literal => [ $( ($part:literal, $reduce:expr) ),* $(,)? ] => $expr:literal) => {
        $set.add_builder($crate::modifier_set::ComplexAttribute::new(
            $name, &[ $( ($part, $reduce) ),* ], $expr,
        ));
    };

    // Builder: @build expr , ...rest
    (@munch $set:ident, @build $builder:expr , $($rest:tt)*) => {
        $set.add_builder($builder);
        $crate::mod_set!(@munch $set, $($rest)*);
    };
    // Builder: @build expr (terminal)
    (@munch $set:ident, @build $builder:expr) => {
        $set.add_builder($builder);
    };

    // Tagged modifier: "attr" [TAG] => value , ...rest
    (@munch $set:ident, $attribute:literal [ $($tag:tt)+ ] => $value:expr , $($rest:tt)*) => {
        $set.add_tagged($attribute, $value, $($tag)+);
        $crate::mod_set!(@munch $set, $($rest)*);
    };
    // Tagged modifier: "attr" [TAG] => value (terminal)
    (@munch $set:ident, $attribute:literal [ $($tag:tt)+ ] => $value:expr) => {
        $set.add_tagged($attribute, $value, $($tag)+);
    };

    // Untagged modifier: "attr" => value , ...rest
    (@munch $set:ident, $attribute:literal => $value:expr , $($rest:tt)*) => {
        $set.add($attribute, $value);
        $crate::mod_set!(@munch $set, $($rest)*);
    };
    // Untagged modifier: "attr" => value (terminal)
    (@munch $set:ident, $attribute:literal => $value:expr) => {
        $set.add($attribute, $value);
    };

    // ── Path-named arms (mirror the literal arms) ───────────────────────
    //
    // These let attribute names be `&str` constants/paths (e.g. `attr::VITALITY`)
    // instead of string literals, so names can be defined once and refactored
    // safely. A string literal never matches `:path` and a path never matches
    // `:literal`, so the two arm-sets don't collide; a bare unquoted name would
    // match here and fail later as an unresolved value. `:path` (unlike `:expr`)
    // is permitted before both `[` and `=>` by the macro follow-set rules.

    // Tagged modifier: PATH [TAG] => value , ...rest
    (@munch $set:ident, $attribute:path [ $($tag:tt)+ ] => $value:expr , $($rest:tt)*) => {
        $set.add_tagged($attribute, $value, $($tag)+);
        $crate::mod_set!(@munch $set, $($rest)*);
    };
    // Tagged modifier: PATH [TAG] => value (terminal)
    (@munch $set:ident, $attribute:path [ $($tag:tt)+ ] => $value:expr) => {
        $set.add_tagged($attribute, $value, $($tag)+);
    };

    // Untagged modifier: PATH => value , ...rest
    (@munch $set:ident, $attribute:path => $value:expr , $($rest:tt)*) => {
        $set.add($attribute, $value);
        $crate::mod_set!(@munch $set, $($rest)*);
    };
    // Untagged modifier: PATH => value (terminal)
    (@munch $set:ident, $attribute:path => $value:expr) => {
        $set.add($attribute, $value);
    };

    // ── Entry point (must be last - $($tt:tt)* matches everything) ──────

    { $($tt:tt)* } => {{
        let mut _set = $crate::modifier_set::ModifierSet::new();
        $crate::mod_set!(@munch _set, $($tt)*);
        _set
    }};
}

/// Create an [`AttributeInitializer`](crate::modifier_set::AttributeInitializer) component
/// from a set of attribute definitions.
///
/// Spawn this alongside [`Attributes`](crate::attributes::Attributes) to
/// have the modifiers automatically applied on spawn.
///
/// Uses the same syntax as [`mod_set!`] - this is just a convenience wrapper
/// that returns an `AttributeInitializer` instead of a bare `ModifierSet`.
///
/// # Example
///
/// ```ignore
/// commands.spawn((
///     Attributes::new(),
///     attributes! {
///         "Strength" => 50.0,
///         "Damage.Added" [FIRE | MELEE] => 10.0,
///         "Health" => "Strength * 2.0",
///         @build ComplexAttribute::new("Health",
///             &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
///             "base * (1 + increased)",
///         ),
///     },
/// ));
/// ```
#[macro_export]
macro_rules! attributes {
    { $($tt:tt)* } => {
        $crate::modifier_set::AttributeInitializer::new($crate::mod_set!{ $($tt)* })
    };
}

#[cfg(test)]
mod tests {
    use crate::modifier_set::ModifierValue;
    use crate::tags::TagMask;

    // Attribute names as `&str` constants/paths, the case the `:path` arms enable.
    const STRENGTH: &str = "Strength";
    const DAMAGE_ADDED: &str = "Damage.Added";

    fn literal(value: &ModifierValue) -> f32 {
        match value {
            ModifierValue::Literal(v) => *v,
            other => panic!("expected literal, got {other:?}"),
        }
    }

    fn expr(value: &ModifierValue) -> &str {
        match value {
            ModifierValue::ExprSource(s) => s,
            other => panic!("expected expr, got {other:?}"),
        }
    }

    #[test]
    fn accepts_literal_and_path_names() {
        const FIRE: TagMask = TagMask::bit(0);

        // String literals and `&str` paths mix freely, tagged and untagged.
        let set = mod_set! {
            "Vitality" => 10.0,
            STRENGTH => 5.0,
            STRENGTH => "Vitality * 2.0",
            DAMAGE_ADDED [FIRE] => 7.0,
        };
        let entries = set.entries();
        assert_eq!(entries.len(), 4);

        assert_eq!(entries[0].attribute, "Vitality");
        assert_eq!(literal(&entries[0].value), 10.0);

        assert_eq!(entries[1].attribute, "Strength");
        assert_eq!(literal(&entries[1].value), 5.0);

        assert_eq!(entries[2].attribute, "Strength");
        assert_eq!(expr(&entries[2].value), "Vitality * 2.0");

        assert_eq!(entries[3].attribute, "Damage.Added");
        assert_eq!(literal(&entries[3].value), 7.0);
        assert_eq!(entries[3].tag, FIRE);
    }
}
