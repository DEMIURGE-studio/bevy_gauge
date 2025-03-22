use std::{cell::UnsafeCell, marker::PhantomData};
use bevy::{ecs::{archetype::Archetype, component::{ComponentId, Components, StorageType, Tick}, query::{FilteredAccess, QueryFilter, WorldQuery}, storage::{ComponentSparseSet, Table, TableRow}, world::unsafe_world_cell::UnsafeWorldCell}, prelude::*, ptr::{ThinSlicePtr, UnsafeCellDeref}};

pub(super) union StorageSwitch<C: Component, T: Copy, S: Copy> {
    /// The table variant. Requires the component to be a table component.
    table: T,
    /// The sparse set variant. Requires the component to be a sparse set component.
    sparse_set: S,
    _marker: PhantomData<C>,
}

impl<C: Component, T: Copy, S: Copy> StorageSwitch<C, T, S> {
    /// Creates a new [`StorageSwitch`] using the given closures to initialize
    /// the variant corresponding to the component's [`StorageType`].
    pub fn new(table: impl FnOnce() -> T, sparse_set: impl FnOnce() -> S) -> Self {
        match C::STORAGE_TYPE {
            StorageType::Table => Self { table: table() },
            StorageType::SparseSet => Self {
                sparse_set: sparse_set(),
            },
        }
    }

    /// Creates a new [`StorageSwitch`] using a table variant.
    ///
    /// # Panics
    ///
    /// This will panic on debug builds if `C` is not a table component.
    ///
    /// # Safety
    ///
    /// `C` must be a table component.
    #[inline]
    pub unsafe fn set_table(&mut self, table: T) {
        match C::STORAGE_TYPE {
            StorageType::Table => self.table = table,
            _ => {
                #[cfg(debug_assertions)]
                unreachable!();
                #[cfg(not(debug_assertions))]
                std::hint::unreachable_unchecked()
            }
        }
    }

    /// Fetches the internal value from the variant that corresponds to the
    /// component's [`StorageType`].
    pub fn extract<R>(&self, table: impl FnOnce(T) -> R, sparse_set: impl FnOnce(S) -> R) -> R {
        match C::STORAGE_TYPE {
            StorageType::Table => table(
                // SAFETY: C::STORAGE_TYPE == StorageType::Table
                unsafe { self.table },
            ),
            StorageType::SparseSet => sparse_set(
                // SAFETY: C::STORAGE_TYPE == StorageType::SparseSet
                unsafe { self.sparse_set },
            ),
        }
    }
}

impl<C: Component, T: Copy, S: Copy> Clone for StorageSwitch<C, T, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: Component, T: Copy, S: Copy> Copy for StorageSwitch<C, T, S> {}

pub struct Dirty<T>(std::marker::PhantomData<T>);

pub struct DirtyFetch<'w, T: Component> {
    ticks: StorageSwitch<T, Option<ThinSlicePtr<'w, UnsafeCell<Tick>>>, &'w ComponentSparseSet>,
    last_run: Tick,
    this_run: Tick,
}

impl<T: Component> Clone for DirtyFetch<'_, T> {
    fn clone(&self) -> Self {
        Self {
            ticks: self.ticks,
            last_run: self.last_run,
            this_run: self.this_run,
        }
    }
}

impl<T> DebugCheckedUnwrap for Option<T> {
    type Item = T;

    #[inline(always)]
    #[track_caller]
    unsafe fn debug_checked_unwrap(self) -> Self::Item {
        if let Some(inner) = self {
            inner
        } else {
            unreachable!()
        }
    }
}

pub(crate) trait DebugCheckedUnwrap {
    type Item;
    /// # Panics
    /// Panics if the value is `None` or `Err`, only in debug mode.
    ///
    /// # Safety
    /// This must never be called on a `None` or `Err` value. This can
    /// only be called on `Some` or `Ok` values.
    unsafe fn debug_checked_unwrap(self) -> Self::Item;
}

/// SAFETY:
/// `fetch` accesses a single component in a readonly way.
/// This is sound because `update_component_access` add read access for that component and panics when appropriate.
/// `update_component_access` adds a `With` filter for a component.
/// This is sound because `matches_component_set` returns whether the set contains that component.
unsafe impl<T: Component> WorldQuery for Dirty<T> {
    type Item<'w> = bool;
    type Fetch<'w> = DirtyFetch<'w, T>;
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
    fn update_component_access(&_id: &ComponentId, _access: &mut FilteredAccess<ComponentId>) {
        // if access.access().has_component_write(id) {
        //     panic!("$state_name<{}> conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",core::any::type_name::<T>());
        // }
        // access.add_component_read(id);
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
unsafe impl<T: Component> QueryFilter for Dirty<T> {
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