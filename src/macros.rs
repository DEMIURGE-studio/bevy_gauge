/// Create a [`ModifierSet`](crate::modifier_set::ModifierSet) from a set
/// of attribute definitions.
///
/// # Syntax
///
/// ```ignore
/// mod_set! {
///     "AttributeName" => value,                         // untagged
///     "AttributeName" [TAG_EXPR] => value,              // tagged
/// }
/// ```
///
/// - **`value`** can be an `f32` literal (becomes a flat modifier) or a
///   `&str` / string literal (becomes an expression modifier compiled at
///   apply time).
/// - **`TAG_EXPR`** is any Rust expression that evaluates to a [`TagMask`].
///   Typically `FIRE | MELEE` or `DamageTags::PHYSICAL`.
///
/// # Example
///
/// ```ignore
/// let buff = mod_set! {
///     "Damage.Increased" => 0.25,
///     "Damage.Added" [FIRE | MELEE] => 10.0,
///     "Health" => "Strength * 2.0",
/// };
/// buff.apply(entity, &mut attributes);
/// ```
#[macro_export]
macro_rules! mod_set {
    { $( $attribute:literal $( [ $tag:expr ] )? => $value:expr ),* $(,)? } => {{
        let mut _set = $crate::modifier_set::ModifierSet::new();
        $(
            $crate::mod_set!(@entry _set, $attribute $(, $tag )?, $value);
        )*
        _set
    }};

    // Internal: entry with tag
    (@entry $set:ident, $attribute:literal, $tag:expr, $value:expr) => {
        $set.add_tagged($attribute, $value, $tag);
    };

    // Internal: entry without tag
    (@entry $set:ident, $attribute:literal, $value:expr) => {
        $set.add($attribute, $value);
    };
}

/// Create an [`AttributeInitializer`](crate::modifier_set::AttributeInitializer) component
/// from a set of attribute definitions.
///
/// Spawn this alongside [`Attributes`](crate::attributes::Attributes) to
/// have the modifiers automatically applied on spawn.
///
/// Uses the same syntax as [`mod_set!`] — this is just a convenience wrapper
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
///     },
/// ));
/// ```
#[macro_export]
macro_rules! attributes {
    { $($tt:tt)* } => {
        $crate::modifier_set::AttributeInitializer::new($crate::mod_set!{ $($tt)* })
    };
}
