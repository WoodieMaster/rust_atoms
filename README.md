# atoms

A Rust library for creating interned, program-lifetime values — called **Atoms**.

## What is an Atom?

An `Atom<T>` is a thin, copyable wrapper around a `&'static T` reference. When you create an `Atom` from a value:

- If an equal value has already been atomized, **you get back a reference to the existing one** — no allocation, no duplication.
- If it's new, the value is **leaked into the global `AtomStore`** and a reference to it is returned.

This means two `Atom`s that are equal by value are _guaranteed_ to point to the exact same memory address, so equality checks become a fast pointer comparison instead of a deep value comparison.

```rust
use atoms::Atom;

let a: Atom<str> = Atom::new("hello");
let b: Atom<str> = Atom::new("hello".to_string());

assert_eq!(a, b); // true — same pointer, O(1) comparison
```

## How it works

Internally, `atoms` maintains a global, lazily-initialized `AtomStore` — a two-level concurrent map:

```
GLOBAL_ATOM_STORE: Map<TypeId, TypedAtomStore<T>>
                              └── Map<&'static T, Atom<T>>
```

1. On the first call for a given `T`, a per-type store (`OnceMap`) is created and inserted into the global store.
2. `Atom::new` checks the per-type store for an existing atom equal to the input.
3. If found, the existing `Atom` is cloned (just copying a pointer) and returned.
4. If not found, the value is promoted to a `&'static T` (via `Box::leak` or `String::leak`) and stored.

All of this is **thread-safe**: the underlying [`once_map`](https://crates.io/crates/once_map) crate provides concurrent insert-once semantics.

## Key properties

| Property                   | Detail                                                     |
| -------------------------- | ---------------------------------------------------------- |
| **`Copy` + `Clone`**       | `Atom<T>` is just a pointer — free to copy                 |
| **`PartialEq` by pointer** | Equality is `O(1)` regardless of `T`'s size                |
| **`Deref<Target = T>`**    | Use an `Atom<T>` anywhere a `&T` is expected               |
| **Thread-safe**            | `Send + Sync` bounds on `T` enforced; store uses `OnceMap` |
| **Program-lifetime**       | Atoms are never freed; memory grows monotonically          |

## Usage

Add `atoms` to your `Cargo.toml`:

```toml
[dependencies]
atoms = { path = "." }
```

### Strings

```rust
use atoms::Atom;

let s1: Atom<str> = Atom::new("foo");          // &'static str path
let s2: Atom<str> = Atom::new("foo".to_string()); // String path

assert!(std::ptr::eq(&*s1, &*s2)); // same allocation
```

### Arbitrary types

```rust
use atoms::Atom;

#[derive(Hash, Eq, PartialEq)]
struct Config { debug: bool }

let a = Atom::new(Config { debug: true });
let b = Atom::new(Config { debug: true });

assert_eq!(a, b); // deduplicated
```

### Slices / Vecs

```rust
use atoms::Atom;

let v1: Atom<[u8]> = Atom::new(vec![1, 2, 3]);
let v2: Atom<[u8]> = Atom::new(vec![1, 2, 3]);

assert_eq!(v1, v2);
```

## The `Atomize` trait

`Atom::new` accepts anything that implements `Atomize<T>`, which converts a value into a `&'static T`. Built-in implementations:

| Input type   | Target `T`             |
| ------------ | ---------------------- |
| `T`          | `T` (boxed and leaked) |
| `Box<T>`     | `T` (leaked directly)  |
| `String`     | `str`                  |
| `Vec<T>`     | `[T]`                  |
| `&'static T` | `T` (no allocation)    |

You can implement `Atomize` for your own smart-pointer or container types.

## When to use `atoms`

- **Interning identifiers / symbols** — e.g. compiler symbol tables, configuration keys.
- **Cheap equality on large values** — once atomized, comparing two atoms is a pointer comparison.
- **Sharing immutable data across threads** — `Atom<T>` is `Send + Sync` without reference counting overhead.

## Caveats

- Memory is **never reclaimed**. Only use atoms for values whose count is bounded over the life of your program.
- `T` must implement `Hash + Eq` (for deduplication) as well as `Send + Sync + 'static`.

## License

MIT
