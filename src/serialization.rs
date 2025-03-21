// use std::collections::HashMap;
// use evalexpr::{DefaultNumericTypes, Node};
// use serde::de::{self, Visitor};
// use serde::{Deserialize, Deserializer};
// use std::fmt;
// use crate::prelude::{Expression, StatCollection, StatRequirement, ValueType};
// 
// /// A wrapper for evalexpr's Node for parsing expressions.
// pub struct ExprWrapper(Node<DefaultNumericTypes>);
// 
// impl From<Node<DefaultNumericTypes>> for ExprWrapper {
//     fn from(value: Node<DefaultNumericTypes>) -> Self {
//         Self(value)
//     }
// }
// 
// fn parse_expression_with_prefix(s: &str) -> Node<DefaultNumericTypes> {
//     let prefixed = format!("Total = {}", s);
//     evalexpr::build_operator_tree(&prefixed)
//         .unwrap_or_else(|e| panic!("Failed to parse expression '{}': {}", prefixed, e))
// }
// 
// impl From<&str> for ExprWrapper {
//     fn from(value: &str) -> Self {
//         // Directly parse here, or just go via `Expression`
//         let node = parse_expression_with_prefix(value);
//         ExprWrapper(node)
//     }
// }
// 
// impl From<String> for ExprWrapper {
//     fn from(value: String) -> Self {
//         let node = parse_expression_with_prefix(&value);
//         ExprWrapper(node)
//     }
// }
// 
// impl From<ExprWrapper> for Node<DefaultNumericTypes> {
//     fn from(value: ExprWrapper) -> Self {
//         value.0.clone()
//     }
// }
// 
// impl From<Node<DefaultNumericTypes>> for Expression {
//     fn from(value: Node<DefaultNumericTypes>) -> Self {
//         Expression(value)
//     }
// }
// 
// impl From<f32> for ValueType {
//     fn from(value: f32) -> Self {
//         Self::from_float(value)
//     }
// }
// 
// impl From<i32> for ValueType {
//     fn from(value: i32) -> Self {
//         Self::from_float(value as f32)
//     }
// }
// 
// impl From<&str> for ValueType {
//     fn from(value: &str) -> Self {
//         Self::from_expression(value.into())
//     }
// }
// 
// impl From<String> for ValueType {
//     fn from(value: String) -> Self {
//         Self::from_expression(value.into())
//     }
// }
// 
// impl From<&str> for Expression {
//     fn from(value: &str) -> Self {
//         Expression(parse_expression_with_prefix(value))
//     }
// }
// 
// impl From<String> for Expression {
//     fn from(value: String) -> Self {
//         Expression(parse_expression_with_prefix(&value))
//     }
// }
// 
// /// Deserialize `StatType` from floats, integers, or string expressions.
// impl<'de> Deserialize<'de> for ValueType {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         struct StatTypeVisitor;
// 
//         impl<'de> Visitor<'de> for StatTypeVisitor {
//             type Value = ValueType;
// 
//             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//                 write!(formatter, "a float, integer, or string containing an expression")
//             }
// 
//             fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
//             where
//                 E: de::Error,
//             {
//                 Ok(ValueType::from_float(value as f32))
//             }
// 
//             fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
//             where
//                 E: de::Error,
//             {
//                 Ok(ValueType::from_float(value as f32))
//             }
// 
//             fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
//             where
//                 E: de::Error,
//             {
//                 Ok(ValueType::from_float(value as f32))
//             }
// 
//             fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
//             where
//                 E: de::Error,
//             {
//                 Ok(ValueType::from_expression(value.into()))
//             }
// 
//             fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
//             where
//                 E: de::Error,
//             {
//                 Ok(ValueType::from_expression(value.into()))
//             }
//         }
// 
//         deserializer.deserialize_any(StatTypeVisitor)
//     }
// }
// 
// impl<'de> Deserialize<'de> for StatCollection {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         // 1) Parse raw JSON into HashMap<String, NestedStatType>
//         let nested_map = HashMap::<String, NestedStatType>::deserialize(deserializer)?;
// 
//         // 2) Flatten into a HashMap<String, StatType>
//         let mut flat_map = HashMap::new();
//         flatten_nested_stats("", &nested_map, &mut flat_map);
// 
//         // 3) Wrap in Stats and return
//         Ok(StatCollection { 0: flat_map})
//     }
// }
// 
// /// Implements `Deserialize` for `StatRequirement`.
// impl<'de> Deserialize<'de> for StatRequirement {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let requirement_str = String::deserialize(deserializer)?;
//         let node = evalexpr::build_operator_tree(&requirement_str)
//             .map_err(|_| serde::de::Error::custom(format!("Failed to parse stat requirement: {}", requirement_str)))?;
//         Ok(StatRequirement(node))
//     }
// }
// 
// #[derive(Debug, Deserialize)]
// #[serde(untagged)]
// enum NestedStatType {
//     Value(ValueType),
//     Map(HashMap<String, NestedStatType>),
// }
// 
// fn flatten_nested_stats(
//     prefix: &str,
//     nested: &HashMap<String, NestedStatType>,
//     out: &mut HashMap<String, ValueType>,
// ) {
//     for (key, value) in nested {
//         // Build the full key: "prefix.key" or just "key" if no prefix
//         let full_key = if prefix.is_empty() {
//             key.to_string()
//         } else {
//             format!("{}.{}", prefix, key)
//         };
// 
//         match value {
//             NestedStatType::Value(stat_type) => {
//                 // Store final leaf
//                 out.insert(full_key, stat_type.clone());
//             }
//             NestedStatType::Map(submap) => {
//                 // Recurse into submap
//                 flatten_nested_stats(&full_key, submap, out);
//             }
//         }
//     }
// }
