// Macro to define all tags and the registry
#[macro_export]
macro_rules! define_tag_system {
    (
        $(
            $group_name:ident {
                $(
                    $tag_name:ident = $bit_pos:expr
                ),* $(,)?
            }
        ),* $(,)?
    ) => {
        use std::collections::HashMap;

        pub struct TagRegistry {
            pub registry: HashMap<String, HashMap<String, u32>>,
            pub reverse_registry: HashMap<String, HashMap<u32, String>>,
        }

        // Define all tag constants
        $(
            $(
                pub const $tag_name: u32 = 1u32 << $bit_pos;
            )*
            
            // Define group constants
            pub const $group_name: u32 = $( $tag_name )|*;
        )*

        impl TagRegistry {
            pub fn new() -> Self {
                let mut registry = HashMap::new();
                let mut reverse_registry = HashMap::new();

                $(
                    let mut group_map = HashMap::new();
                    let mut reverse_group_map = HashMap::new();

                    $(
                        group_map.insert(stringify!($tag_name).to_string(), $tag_name);
                        reverse_group_map.insert($tag_name, stringify!($tag_name).to_string());
                    )*

                    registry.insert(stringify!($group_name).to_string(), group_map);
                    reverse_registry.insert(stringify!($group_name).to_string(), reverse_group_map);
                )*

                TagRegistry {
                    registry,
                    reverse_registry,
                }
            }

            // Helper method to get tag value by name and group
            pub fn get_tag_value(&self, group_name: &str, tag_name: &str) -> Option<u32> {
                self.registry
                    .get(group_name)
                    .and_then(|group| group.get(tag_name))
                    .copied()
            }

            // Helper method to get tag name by value and group
            pub fn get_tag_name(&self, group_name: &str, tag_value: u32) -> Option<&String> {
                self.reverse_registry
                    .get(group_name)
                    .and_then(|group| group.get(&tag_value))
            }

            // Helper method to check if a tag value belongs to a group
            pub fn is_in_group(&self, tag_value: u32, group_name: &str) -> bool {
                if let Some(group_const) = self.get_group_mask(group_name) {
                    (tag_value & group_const) != 0
                } else {
                    false
                }
            }

            // Helper method to get the combined mask for a group
            pub fn get_group_mask(&self, group_name: &str) -> Option<u32> {
                self.registry.get(group_name).map(|group| {
                    group.values().fold(0u32, |acc, &val| acc | val)
                })
            }
        }
    }
}

// This is a helper macro to define tags in a specific sequence within their group
#[macro_export]
macro_rules! define_tags {
    (
        $(
            $group_name:ident {
                $($tag_name:ident),* $(,)?
            }
        ),* $(,)?
    ) => {
        define_tag_system! {
            $(
                $group_name {
                    $($tag_name = __count_prev!($($tag_name),*, $tag_name)),*
                }
            ),*
        }
    };
}

// Helper macro to count position in a list
#[macro_export]
macro_rules! __count_prev {
    // If the current item matches the target, return 0
    ($target:ident, $current:ident $(, $rest:ident)*) => {
        {
            const POSITION: usize = {
                let mut pos = 0;
                $(
                    if stringify!($current) != stringify!($target) && 
                       stringify!($rest) != stringify!($target) {
                        pos += 1;
                    }
                )*
                pos
            };
            POSITION
        }
    };
}

// Example usage
define_tag_system! {
    DAMAGE_TYPE {
        FIRE = 0,
        COLD = 1,
        LIGHTNING = 2,
        PHYSICAL = 3,
        CHAOS = 4
    },
    WEAPON_TYPE {
        SWORD = 0,
        AXE = 1,
        HAMMER = 2,
        PISTOL = 3,
        RIFLE = 4,
        CLAW = 5,
        DAGGER = 6,
        STAFF = 7,
        WAND = 8,
        BOW = 9,
        SCEPTER = 10
    },
    RESOURCE {
        MIN_HEALTH = 0,
        MAX_HEALTH = 1,
        MIN_MANA = 2,
        MAX_MANA = 3,
        MIN_STAMINA = 4,
        MAX_STAMINA = 5,
        FRENZY_CHARGES = 6,
        ENDURANCE_CHARGES = 7
    }
}
pub const ELEMENT: u32 = FIRE | COLD | LIGHTNING;
pub const RANGED_WEAPON: u32 = PISTOL | RIFLE | WAND | BOW;
pub const MELEE_WEAPON: u32 = SWORD | AXE | HAMMER | CLAW | DAGGER | STAFF | SCEPTER;
pub const SLASHING: u32 = AXE | SWORD | CLAW;
pub const PRIMARY_RESOURCE: u32 = MAX_HEALTH | MAX_MANA | MAX_STAMINA;

// PRIMARY -< GROUPS -< TYPES
// STRING/HASH -> STRING/HASH -> BITAND COMPARE bits in each type

// 11111 -> 1110


// TARGET TAG - Damage of some ability
// damage.fire.hammer|axe.one_handed - 100_101_01
// resource.max_life
// resource.max_mana

// QUERY is made, we treat all non-specified target groups as zero

// MODIFIER TAG
// damage.elemental.sword    - 111_100_XX_XXXXX // APPLIES
// damage elemental.melee    - 111_111_XX_XXXXX // APPLIES
// damage elemental.melee.1h - 111_111_01_XXXXX // APPLIES
// damage elemental.sword.1h - 111_010_01_XXXXX // APPLIES
// damage elemental.melee.2h - 111_111_10_XXXXX // FAILS
// damage melee.1h.warrior   - XXX_111_01_00001 // FAILS
