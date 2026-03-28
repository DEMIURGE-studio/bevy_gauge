/// Create a [`ModifierSet`](crate::modifier_set::ModifierSet) from a set
/// of attribute definitions.
///
/// # Syntax
///
/// ```ignore
/// mod_set! {
///     "AttributeName" => value,                         // untagged modifier
///     "AttributeName" [TAG_EXPR] => value,              // tagged modifier
///     @build ComplexAttribute::new(...),                 // attribute builder
/// }
/// ```
///
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
