use std::cell::UnsafeCell;

use bevy::{prelude::*, ptr::ThinSlicePtr};
use super::prelude::*;

pub(crate) fn add_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<Entity, (Changed<Stats>, Without<T>)>,
    stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    for entity in stats_query.iter_mut() {
        let stats = stat_accessor.build(entity);
        if T::is_valid(&stats) {
            commands.entity(entity).insert(T::from_stats(&stats));
        }
    }
}

pub(crate) fn update_stat_component_system<T: StatDerived + Component>(
    mut stats_query: Query<(Entity, &mut T), Changed<Stats>>,
    stat_accessor: StatAccessor,
    mut commands: Commands,
) {
    for (entity, mut stat_component) in stats_query.iter_mut() {
        let stats = stat_accessor.build(entity);
        if stat_component.should_update(&stats) {
            stat_component.update_from_stats(&stats);
        }
        if !T::is_valid(&stats) {
            commands.entity(entity).remove::<T>();
        }
    }
}

pub(crate) fn update_writeback_value_system<T: WriteBack + Component>(
    mut stats_query: Query<(&mut Stats, &T), Changed<T>>,
) {
    for (mut stat_component, writeback) in stats_query.iter_mut() {
        writeback.write_back(&mut stat_component);
    }
}










use bevy_ecs::{archetype::Archetype, component::{ComponentId, Components, StorageType, Tick}, query::*, storage::{ComponentSparseSet, Table, TableRow}, world::unsafe_world_cell::UnsafeWorldCell};

pub struct Changed<T>(std::marker::PhantomData<T>);

#[derive(Clone)]
pub struct ChangedFetch<'w, T: Component> {
    ticks: StorageSwitch<T, Option<ThinSlicePtr<'w, UnsafeCell<Tick>>>, &'w ComponentSparseSet>,
    last_run: Tick,
    this_run: Tick,
}

/// SAFETY:
/// `fetch` accesses a single component in a readonly way.
/// This is sound because `update_component_access` add read access for that component and panics when appropriate.
/// `update_component_access` adds a `With` filter for a component.
/// This is sound because `matches_component_set` returns whether the set contains that component.
unsafe impl<T: Component> WorldQuery for Changed<T> {
    type Item<'w> = bool;
    type Fetch<'w> = ChangedFetch<'w, T>;
    type State = ComponentId;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        item
    }

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        &id: &ComponentId,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        Self::Fetch::<'w> {
            ticks: StorageSwitch::new(
                || None,
                || {
                    // SAFETY: The underlying type associated with `component_id` is `T`,
                    // which we are allowed to access since we registered it in `update_archetype_component_access`.
                    // Note that we do not actually access any components' ticks in this function, we just get a shared
                    // reference to the sparse set, which is used to access the components' ticks in `Self::fetch`.
                    unsafe { world.storages().sparse_sets.get(id).debug_checked_unwrap() }
                },
            ),
            last_run,
            this_run,
        }
    }

    const IS_DENSE: bool = {
        match T::STORAGE_TYPE {
            StorageType::Table => true,
            StorageType::SparseSet => false,
        }
    };

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        component_id: &ComponentId,
        _archetype: &'w Archetype,
        table: &'w Table,
    ) {
        if Self::IS_DENSE {
            // SAFETY: `set_archetype`'s safety rules are a super set of the `set_table`'s ones.
            unsafe {
                Self::set_table(fetch, component_id, table);
            }
        }
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        &component_id: &ComponentId,
        table: &'w Table,
    ) {
        let table_ticks = Some(
            table
                .get_changed_ticks_slice_for(component_id)
                .debug_checked_unwrap()
                .into(),
        );
        // SAFETY: set_table is only called when T::STORAGE_TYPE = StorageType::Table
        unsafe { fetch.ticks.set_table(table_ticks) };
    }

    #[inline(always)]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        fetch.ticks.extract(
            |table| {
                // SAFETY: set_table was previously called
                let table = unsafe { table.debug_checked_unwrap() };
                // SAFETY: The caller ensures `table_row` is in range.
                let tick = unsafe { table.get(table_row.as_usize()) };

                tick.deref().is_newer_than(fetch.last_run, fetch.this_run)
            },
            |sparse_set| {
                // SAFETY: The caller ensures `entity` is in range.
                let tick = unsafe {
                    ComponentSparseSet::get_changed_tick(sparse_set, entity).debug_checked_unwrap()
                };

                tick.deref().is_newer_than(fetch.last_run, fetch.this_run)
            },
        )
    }

    #[inline]
    fn update_component_access(&id: &ComponentId, access: &mut FilteredAccess<ComponentId>) {
        if access.access().has_component_write(id) {
            panic!("$state_name<{}> conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",core::any::type_name::<T>());
        }
        access.add_component_read(id);
    }

    fn init_state(world: &mut World) -> ComponentId {
        world.register_component::<T>()
    }

    fn get_state(components: &Components) -> Option<ComponentId> {
        components.component_id::<T>()
    }

    fn matches_component_set(
        &id: &ComponentId,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        set_contains_id(id)
    }
}

// SAFETY: WorldQuery impl performs only read access on ticks
unsafe impl<T: Component> QueryFilter for Changed<T> {
    const IS_ARCHETYPAL: bool = false;

    #[inline(always)]
    unsafe fn filter_fetch(
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        // SAFETY: The invariants are uphold by the caller.
        unsafe { Self::fetch(fetch, entity, table_row) }
    }
}