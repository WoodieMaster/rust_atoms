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

impl<T: 'static + Send + Sync + Hash + Eq> Atom<T> {
    pub fn new(item: T) -> Self {
        let typed_store =
            GLOBAL_ATOM_STORE.insert(item.type_id(), |_| Box::new(TypedAtomStore::<T>::new()));

        let typed_store: &TypedAtomStore<T> = typed_store
            .downcast_ref()
            // it is always this store type for every atom type
            .unwrap();

        match typed_store.get_cloned(&item) {
            Some(v) => v,
            None => {
                let v_ref: &'static T = Box::leak(Box::new(item));
                typed_store.insert_cloned(v_ref, |_| Atom(v_ref))
            }
        }
    }
}

impl<T: 'static + ?Sized + Send + Sync + Hash + Eq> Atom<T> {
    pub fn from_static(item: &'static T) -> Self {
        let typed_store =
            GLOBAL_ATOM_STORE.insert(item.type_id(), |_| Box::new(TypedAtomStore::<T>::new()));

        let typed_store: &TypedAtomStore<T> = typed_store
            .downcast_ref()
            // it is always this store type for every atom type
            .unwrap();

        typed_store.insert_cloned(item, |_| Atom(item))
    }

    pub fn from_wrapper<W: Atomize<Out = T>>(wrapper: W) -> Self {
        Self::from_static(wrapper.atomize())
    }
}

pub trait Atomize {
    type Out: ?Sized;

    fn atomize(self) -> &'static Self::Out;
}

impl<T> Atomize for Box<T> {
    type Out = T;

    fn atomize(self) -> &'static Self::Out {
        Box::leak(self)
    }
}

impl Atomize for String {
    type Out = str;

    fn atomize(self) -> &'static Self::Out {
        self.leak()
    }
}

impl<T> Atomize for Vec<T> {
    type Out = [T];

    fn atomize(self) -> &'static Self::Out {
        Vec::leak(self)
    }
}
