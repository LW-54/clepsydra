extern crate alloc;

use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ARENA_ID: AtomicUsize = AtomicUsize::new(0);

#[macro_export]
macro_rules! new_arena {
    () => {{
        struct Brand<'brand>(::core::marker::PhantomData<fn(&'brand ()) -> &'brand ()>);
        $crate::arena::Arena::new(Brand(::core::marker::PhantomData))
    }};
}

#[derive(Debug)]
pub struct ArenaId<'brand, T> {
    index: usize,
    owner: usize,
    _brand: core::marker::PhantomData<fn(&'brand ()) -> &'brand ()>,
    _marker: core::marker::PhantomData<T>,
}

impl<T> Clone for ArenaId<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ArenaId<'_, T> {}

impl<T> PartialEq for ArenaId<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && self.index == other.index
    }
}
impl<T> Eq for ArenaId<'_, T> {}

impl<T> Hash for ArenaId<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.owner.hash(state);
        self.index.hash(state);
    }
}

#[derive(Debug)]
pub struct Arena<'brand, T> {
    data: Vec<T>,
    owner: usize,
    _brand: core::marker::PhantomData<fn(&'brand ()) -> &'brand ()>,
}

impl<'brand, T> Arena<'brand, T> {
    pub fn new<B>(_brand: B) -> Self {
        Self {
            data: Vec::new(),
            owner: NEXT_ARENA_ID.fetch_add(1, Ordering::Relaxed),
            _brand: PhantomData,
        }
    }

    pub fn push(&mut self, item: T) -> ArenaId<'brand, T> {
        let id = ArenaId {
            index: self.data.len(),
            owner: self.owner,
            _brand: PhantomData,
            _marker: PhantomData,
        };
        self.data.push(item);
        id
    }

    #[allow(clippy::unreachable)]
    #[must_use]
    /// # Panics
    ///
    /// Panics if the ID belongs to a different arena or has an invalid index.
    pub fn get(&self, id: ArenaId<'brand, T>) -> &T {
        assert_eq!(id.owner, self.owner, "ArenaId belongs to another arena");
        self.data
            .get(id.index)
            .unwrap_or_else(|| unreachable!("ArenaId index is invalid for its arena"))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (ArenaId<'brand, T>, &T)> {
        self.data.iter().enumerate().map(|(i, item)| {
            let id = ArenaId {
                index: i,
                owner: self.owner,
                _brand: PhantomData,
                _marker: PhantomData,
            };
            (id, item)
        })
    }
}
