use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::Hash,
    ops::Deref,
    ptr,
    sync::LazyLock,
};

use once_map::OnceMap;

type TypedStoreWrapper = Box<dyn Any + Send + Sync>;

type TypeStore = OnceMap<TypeId, TypedStoreWrapper>;

static GLOBAL_ATOM_STORE: LazyLock<TypeStore> = LazyLock::new(|| OnceMap::new());

type TypedAtomStore<T> = OnceMap<&'static T, Atom<T>>;

#[derive(Copy)]
pub struct Atom<T: 'static + ?Sized + Send + Sync>(&'static T);

impl<T: 'static + Send + Sync + Debug + ?Sized> Debug for Atom<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Atom").field(&self.0).finish()
    }
}

impl<T: 'static + ?Sized + Send + Sync> PartialEq for Atom<T> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.0, other.0)
    }
}

impl<T: 'static + ?Sized + Send + Sync> Clone for Atom<T> {
    fn clone(&self) -> Self {
        return Self(self.0);
    }
}

impl<T: 'static + ?Sized + Send + Sync> Deref for Atom<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T: 'static + Send + Sync + Hash + Eq + ?Sized> Atom<T> {
    pub fn new<A: Atomize<T>>(item: A) -> Self {
        let typed_store =
            GLOBAL_ATOM_STORE.insert(TypeId::of::<T>(), |_| Box::new(TypedAtomStore::<T>::new()));

        let typed_store: &TypedAtomStore<T> = typed_store
            .downcast_ref()
            // it is always this store type for every atom type
            .unwrap();

        match typed_store.get_cloned(item.atom_ref()) {
            Some(v) => v,
            None => {
                let v_ref: &'static T = item.atomize();
                typed_store.insert_cloned(v_ref, |_| Atom(v_ref))
            }
        }
    }
}

pub trait Atomize<T: ?Sized> {
    fn atom_ref(&self) -> &T;
    fn atomize(self) -> &'static T;
}

impl<T> Atomize<T> for Box<T> {
    #[inline]
    fn atomize(self) -> &'static T {
        Box::leak(self)
    }

    #[inline]
    fn atom_ref(&self) -> &T {
        self.as_ref()
    }
}

impl Atomize<str> for String {
    #[inline]
    fn atomize(self) -> &'static str {
        self.leak()
    }

    #[inline]
    fn atom_ref(&self) -> &str {
        self.as_ref()
    }
}

impl<T> Atomize<[T]> for Vec<T> {
    #[inline]
    fn atomize(self) -> &'static [T] {
        Vec::leak(self)
    }

    #[inline]
    fn atom_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> Atomize<T> for T {
    #[inline]
    fn atomize(self) -> &'static T {
        Box::leak(Box::new(self))
    }

    #[inline]
    fn atom_ref(&self) -> &T {
        self
    }
}

impl<T: ?Sized> Atomize<T> for &'static T {
    #[inline]
    fn atomize(self) -> &'static T {
        self
    }

    #[inline]
    fn atom_ref(&self) -> &T {
        *self
    }
}
