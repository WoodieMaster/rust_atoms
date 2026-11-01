//! # atoms
//!
//! A library for interning values into program-lifetime singletons called **Atoms**.
//!
//! An [`Atom<T>`] is a cheap, [`Copy`]able handle to a value that has been promoted into
//! static memory.  The key guarantee is **pointer identity**: two atoms that are equal by
//! value always wrap the *exact same pointer*, so equality checks are `O(1)` pointer
//! comparisons regardless of how large `T` is.
//!
//! ## How it works
//!
//! On the first call for a given type `T`, a per-type [`OnceMap`][once_map::OnceMap] is
//! lazily created inside a global store keyed by [`TypeId`].  When you call
//! [`Atom::new`], the library:
//!
//! 1. Looks up the per-type store (creating it if needed).
//! 2. Checks whether an equal value is already stored.
//!    - **Hit** – clones the existing `Atom` (just copying a pointer) and returns it.
//!    - **Miss** – leaks the value into static memory via [`Atomize::atomize`], inserts
//!      it, and returns a fresh `Atom`.
//!
//! All operations are **thread-safe**: the underlying `once_map` crate provides
//! concurrent insert-once semantics.
//!
//! ## Example
//!
//! ```rust
//! use atoms::Atom;
//!
//! // Both atoms hold the same interned pointer even though they were created
//! // from two independent heap allocations.
//! let a: Atom<str> = Atom::new("hello".to_string());
//! let b: Atom<str> = Atom::new("hello".to_string());
//!
//! assert_eq!(a, b);                                     // O(1) pointer comparison
//! assert!(std::ptr::eq(&*a as *const str, &*b as *const str)); // same allocation
//! ```
//!
//! ## Memory note
//!
//! Atoms are **never freed**.  The global store grows monotonically for the lifetime of
//! the process.  Only use atoms for values whose cardinality is bounded over the life of
//! your program (e.g. interned identifiers, config keys).

use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::Hash,
    ops::Deref,
    ptr,
    sync::LazyLock,
};

use once_map::OnceMap;

/// Type-erased container for a per-type atom store, held inside the global store.
type TypedStoreWrapper = Box<dyn Any + Send + Sync>;

/// The global top-level store: maps each `TypeId` to its type-erased per-type store.
type TypeStore = OnceMap<TypeId, TypedStoreWrapper>;

/// The single, lazily-initialized global atom store.
///
/// Keyed by [`TypeId`]; each entry is a [`TypedAtomStore<T>`] for the corresponding `T`.
static GLOBAL_ATOM_STORE: LazyLock<TypeStore> = LazyLock::new(|| OnceMap::new());

/// Per-type intern table: maps a `&'static T` to the [`Atom<T>`] that wraps it.
type TypedAtomStore<T> = OnceMap<&'static T, Atom<T>>;

/// An interned, program-lifetime reference to a value of type `T`.
///
/// `Atom<T>` is a thin newtype around `&'static T`.  Its defining property is that two
/// atoms are **equal if and only if they point to the same address** — meaning the
/// global store deduplicates values so that any two equal `T`s map to one allocation.
///
/// # Properties
///
/// | Trait | Behaviour |
/// |---|---|
/// | [`Copy`] / [`Clone`] | Free — just copies a pointer. |
/// | [`PartialEq`] | `O(1)` pointer comparison. |
/// | [`Deref<Target = T>`][Deref] | Transparent access to the underlying value. |
/// | [`Send`] + [`Sync`] | Inherited from `T`'s bounds. |
///
/// # Creating atoms
///
/// Use [`Atom::new`] with anything that implements [`Atomize<T>`]:
///
/// ```rust
/// use atoms::Atom;
///
/// let s1: Atom<str>  = Atom::new("hello".to_string()); // from String
/// let s2: Atom<str>  = Atom::new("hello");             // from 'static reference
/// let n: Atom<u32>   = Atom::new(42_u32);              // from owned value
/// let v: Atom<[u8]>  = Atom::new(vec![1, 2, 3]);       // from Vec
/// ```
#[derive(Copy, Eq)]
pub struct Atom<T: 'static + ?Sized + Send + Sync>(&'static T);

impl<T: 'static + Send + Sync + Debug + ?Sized> Debug for Atom<T> {
    /// Formats the atom as `Atom(<inner>)`, delegating to `T`'s [`Debug`] impl.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Atom").field(&self.0).finish()
    }
}

impl<T: 'static + ?Sized + Send + Sync> PartialEq for Atom<T> {
    /// Returns `true` if both atoms point to the **same address**.
    ///
    /// Because the global store guarantees that equal values share one allocation,
    /// this is equivalent to a deep equality check but runs in `O(1)`.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.0, other.0)
    }
}

impl<T: 'static + ?Sized + Send + Sync> Clone for Atom<T> {
    /// Clones the atom by copying its inner pointer
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T: 'static + ?Sized + Send + Sync> Deref for Atom<T> {
    type Target = T;

    /// Dereferences to the underlying `&T`, giving transparent access to the value.
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T: 'static + Send + Sync + Hash + Eq + ?Sized> Atom<T> {
    /// Interns `item`, returning an [`Atom<T>`] that wraps a program-lifetime reference.
    ///
    /// If an equal value has already been atomized, the existing atom is returned
    /// **without any new allocation**.  If this is the first time this value is seen,
    /// `item` is promoted to `&'static T` via [`Atomize::atomize`] and stored.
    ///
    /// # Type parameters
    ///
    /// - `A` — any type that implements [`Atomize<T>`].  The standard implementations
    ///   cover `T` itself, [`Box<T>`], [`String`] → `str`, [`Vec<T>`] → `[T]`, and
    ///   `&'static T`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atoms::Atom;
    ///
    /// // String interning — two allocations, one atom.
    /// let a: Atom<str> = Atom::new("world".to_string());
    /// let b: Atom<str> = Atom::new("world".to_string());
    /// assert_eq!(a, b);
    ///
    /// // Owned value interning.
    /// let x: Atom<u32> = Atom::new(7_u32);
    /// let y: Atom<u32> = Atom::new(7_u32);
    /// assert_eq!(x, y);
    /// ```
    ///
    /// # Panics
    ///
    /// Never panics under normal use.  The internal `downcast_ref` is guaranteed to
    /// succeed because the global store always stores a `TypedAtomStore<T>` for `TypeId::of::<T>()`.
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

/// Conversion trait for types that can be promoted into a program-lifetime reference.
///
/// Implement this trait to teach [`Atom::new`] how to intern your own container or
/// smart-pointer types.  The split between [`atom_ref`][Atomize::atom_ref] and
/// [`atomize`][Atomize::atomize] is intentional: `atom_ref` lets the library check the
/// cache *without consuming* the value, and `atomize` is only called on a cache miss.
///
/// # Implementing `Atomize`
///
/// Use a local newtype wrapper to satisfy the orphan rule:
///
/// ```rust
/// use atoms::{Atom, Atomize};
///
/// /// A simple reference-counted wrapper around a value.
/// struct Rc<T>(std::rc::Rc<T>);
///
/// impl<T: Clone + std::hash::Hash + Eq + Send + Sync + 'static> Atomize<T> for Rc<T> {
///     fn atom_ref(&self) -> &T {
///         self.0.as_ref()
///     }
///
///     fn atomize(self) -> &'static T {
///         // On a cache miss, clone out of the Rc and box-leak.
///         Box::leak(Box::new((*self.0).clone()))
///     }
/// }
///
/// let a: Atom<u32> = Atom::new(Rc(std::rc::Rc::new(99_u32)));
/// assert_eq!(*a, 99);
/// ```
///
/// # Contract
///
/// - `atom_ref` and `atomize` must refer to the **same logical value**: the reference
///   returned by `atom_ref` must compare equal (via `T: Eq`) to the value that
///   `atomize` would leak.
/// - `atomize` is called **at most once** per unique value — the result is stored in the
///   global atom store and returned on all subsequent lookups.
pub trait Atomize<T: ?Sized> {
    /// Borrows a reference to the underlying value **without consuming** `self`.
    ///
    /// This reference is used to probe the intern table.  It is only valid for the
    /// lifetime of `self`; if a cache miss occurs, [`atomize`][Atomize::atomize] is
    /// called to produce the permanent `&'static T`.
    fn atom_ref(&self) -> &T;

    /// Consumes `self` and returns a `&'static T` suitable for storage.
    ///
    /// Implementations typically achieve this by leaking the underlying heap allocation
    /// (e.g. [`Box::leak`], [`String::leak`], [`Vec::leak`]).  This method is only
    /// called on a cache **miss**; on a hit, the existing static reference is reused.
    fn atomize(self) -> &'static T;
}

/// Interns a [`Box<T>`] by leaking it directly, avoiding an extra copy.
///
/// ```rust
/// use atoms::Atom;
///
/// let boxed = Box::new(42_i32);
/// let atom: Atom<i32> = Atom::new(boxed);
/// assert_eq!(*atom, 42);
/// ```
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

/// Interns a [`Vec<T>`] as a `[T]` atom by leaking its heap buffer.
///
/// ```rust
/// use atoms::Atom;
///
/// let a: Atom<[u8]> = Atom::new(vec![1, 2, 3]);
/// let b: Atom<[u8]> = Atom::new(&[1, 2, 3]);
/// assert_eq!(a, b);
/// ```
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

/// Interns any owned value `T` by boxing and leaking it.
///
/// This is the blanket implementation that covers all `Sized` types not covered by a
/// more specific impl (e.g. [`String`] and [`Vec<T>`]).
///
/// ```rust
/// use atoms::Atom;
///
/// #[derive(Hash, Eq, PartialEq, Debug)]
/// struct Point { x: i32, y: i32 }
///
/// let a = Atom::new(Point { x: 1, y: 2 });
/// let b = Atom::new(Point { x: 1, y: 2 });
/// assert_eq!(a, b);
/// ```
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

/// Pass-through implementation for values that are already `&'static T`.
///
/// No allocation occurs — the reference is stored as-is.  Useful for string literals
/// and other values with a provably static lifetime.
///
/// ```rust
/// use atoms::Atom;
///
/// let s: &'static str = "I am already static";
/// let atom: Atom<str> = Atom::new(s);
/// assert_eq!(&*atom, s);
/// ```
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
