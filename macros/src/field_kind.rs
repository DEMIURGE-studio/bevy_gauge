use syn::Type;

/// How a field converts to and from `f32`, gauge's native value type.
#[derive(Clone, Copy, PartialEq)]
pub enum FieldKind {
    /// `f32`, `f64` — `as` casts
    Float,
    /// Integer types — `.round() as T` to read, `as f32` to write
    Integer,
    /// `bool` — `!= 0.0` to read, `1.0`/`0.0` to write
    Bool,
    /// Anything else — delegate to `AttributeResolvable`
    Composite,
}

/// Recognizes primitives written bare (`f32`) or by their full path
/// (`std::primitive::f32`, `core::primitive::f32`). Anything else, including a
/// type alias for a primitive, is `Composite` and goes through the
/// `AttributeResolvable` impl for that type, which is correct, just not inlined.
pub fn classify_type(ty: &Type) -> FieldKind {
    let Type::Path(type_path) = ty else {
        return FieldKind::Composite;
    };
    if type_path.qself.is_some() {
        return FieldKind::Composite;
    }
    let segments: Vec<String> = type_path
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    let name = match segments.as_slice() {
        [name] => name.as_str(),
        [krate, module, name] if (krate == "std" || krate == "core") && module == "primitive" => {
            name.as_str()
        }
        _ => return FieldKind::Composite,
    };
    match name {
        "f32" | "f64" => FieldKind::Float,
        "bool" => FieldKind::Bool,
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64"
        | "i128" | "isize" => FieldKind::Integer,
        _ => FieldKind::Composite,
    }
}
