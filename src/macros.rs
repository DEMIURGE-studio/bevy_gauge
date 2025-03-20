

#[macro_export]
macro_rules! simple_generic_stat {
    ($struct_name:ident, $ty:ty) => {
        impl StatDerived for $struct_name<$ty> {
            fn from_stats(stats: &bevy_gauge::prelude::StatContextRefs) -> Self {
                let mut s = Self::default();
                s.update_from_stats(stats);
                s
            }

            fn should_update(&self, stats: &bevy_gauge::prelude::StatContextRefs) -> bool {
                stats
                    .get(concat!(stringify!($struct_name), "<", stringify!($ty), ">"))
                    .unwrap_or(0.0)
                    != self.0
            }

            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::StatContextRefs) {
                self.0 = stats
                    .get(concat!(stringify!($struct_name), "<", stringify!($ty), ">"))
                    .unwrap_or(0.0);
            }

            fn is_valid(stats: &bevy_gauge::prelude::StatContextRefs) -> bool {
                stats
                    .get(concat!(stringify!($struct_name), "<", stringify!($ty), ">"))
                    .unwrap_or(0.0)
                    != 0.0
            }
        }
    };
}

#[macro_export]
macro_rules! simple_stat {
    ($struct_name:ident) => {
        impl bevy_gauge::prelude::StatDerived for $struct_name {
            fn from_stats(stats: &bevy_gauge::prelude::StatContextRefs) -> Self {
                let mut s = Self::default();
                s.update_from_stats(stats);
                s
            }

            fn should_update(&self, stats: &bevy_gauge::prelude::StatContextRefs) -> bool {
                stats
                    .get(stringify!($struct_name))
                    .unwrap_or(0.0)
                    != self.0
            }

            fn update_from_stats(&mut self, stats: &bevy_gauge::prelude::StatContextRefs) {
                self.0 = stats
                    .get(stringify!($struct_name))
                    .unwrap_or(0.0);
            }

            fn is_valid(stats: &bevy_gauge::prelude::StatContextRefs) -> bool {
                stats
                    .get(stringify!($struct_name))
                    .unwrap_or(0.0)
                    != 0.0
            }
        }
    };
}

#[macro_export]
macro_rules! stats {
    ( $( $key:expr => $value:expr ),* $(,)? ) => {{
         // Ensure that you bring the required traits into scope.
         use bevy_gauge::prelude::*;
         let mut map = ::bevy_utils::HashMap::new();
         $(
            map.insert($key.to_string(), $value.into());
         )*
         Stats(map)
    }};
}

#[macro_export]
macro_rules! stat_effect {
    ( $( $key:expr => $value:expr ),* $(,)? ) => {{
         // Ensure that you bring the required traits into scope.
         use bevy_gauge::prelude::*;
         let mut map = ::bevy_utils::HashMap::new();
         $(
            map.insert($key.to_string(), $value.into());
         )*
         StatEffect(Stats(map))
    }};
}

#[macro_export]
macro_rules! requires {
    ( $( $key:expr ),* $(,)? ) => {{
         // Ensure that you bring the required traits into scope.
         use bevy_gauge::prelude::*;
         let mut vals = Vec::new();
         $(
            vals.push($key.into());
         )*
         StatRequirements(vals)
    }};
}