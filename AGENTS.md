# AGENTS.md

## Project overview

`atoms` is a small Rust library that interns values into program-lifetime singletons called **Atoms**. The core idea:

- `Atom<T>` wraps a `&'static T` and guarantees deduplication: two atoms with equal values share the same pointer.
- Equality is therefore a pointer comparison — `O(1)` regardless of value size.
- The global `AtomStore` is a two-level concurrent `OnceMap`: `TypeId → (value → Atom<T>)`.

(\*runtime dependency: [`once_map`](https://crates.io/crates/once_map) for concurrent insert-once maps)

## Repository layout

```
src/
  lib.rs    — the entire library (Atom<T>, AtomStore, Atomize trait)
  main.rs   — a small integration demo (not part of the published API)
Cargo.toml
```

## Key types

| Symbol              | File         | Purpose                                           |
| ------------------- | ------------ | ------------------------------------------------- |
| `Atom<T>`           | `src/lib.rs` | Public newtype wrapper around `&'static T`        |
| `GLOBAL_ATOM_STORE` | `src/lib.rs` | Global `LazyLock<OnceMap<TypeId, …>>`             |
| `TypedAtomStore<T>` | `src/lib.rs` | Per-type alias: `OnceMap<&'static T, Atom<T>>`    |
| `Atomize<T>`        | `src/lib.rs` | Trait — converts an owned value into `&'static T` |

## Design invariants — never violate these

1. **Pointer identity = value equality.** If two `Atom<T>` values are equal by `PartialEq`, their inner `&'static T` pointers must be identical. Never store two copies of an equal value.
2. **No deallocation.** Atoms are intentionally leaked. Do not add drop glue, `Arc`, or ref-counting without a very strong reason.
3. **Thread safety.** Every `T` used with `Atom<T>` must be `Send + Sync + 'static`. Do not relax these bounds.
4. **`OnceMap` insert-once semantics.** The global store must never overwrite an existing entry. Always use `insert` / `insert_cloned` (not `insert_or_modify`) for atom storage.
5. **Single global store.** There is exactly one `GLOBAL_ATOM_STORE`. Do not create per-module or per-thread stores.

## How `Atom::new` works (step-by-step)

```
Atom::new(item: A)  where A: Atomize<T>
  │
  ├─ 1. Get (or lazily create) the TypedAtomStore<T> from GLOBAL_ATOM_STORE
  │       keyed by TypeId::of::<T>()
  │
  ├─ 2. Call item.atom_ref() to get a &T without consuming `item`
  │
  ├─ 3. Look up that &T in the TypedAtomStore<T>
  │       Hit  → clone the stored Atom<T> (just copies the pointer) and return
  │       Miss → fall through
  │
  └─ 4. Call item.atomize() to produce a &'static T (may allocate/leak)
         Insert Atom(&'static T) into TypedAtomStore<T>
         Return the new Atom
```

## Adding new `Atomize` implementations

To allow a new container type (e.g. `Arc<T>`) to be atomized, implement `Atomize<T>`:

```rust
impl<T: Clone> Atomize<T> for Arc<T> {
    fn atom_ref(&self) -> &T { self.as_ref() }
    fn atomize(self) -> &'static T { Box::leak(Box::new((*self).clone())) }
}
```

Rules:

- `atom_ref` must **not** consume `self` and must return a reference to the same logical value that `atomize` would leak.
- `atomize` is only called on a **cache miss**, so allocations here are acceptable.
- Implement for new wrapper types, not for `T` itself (already covered by the blanket `impl<T> Atomize<T> for T`).

## Testing

There are currently no automated tests. When adding tests:

- Place unit tests in `src/lib.rs` under `#[cfg(test)]`.
- Key things to test:
  - Two atoms created from equal values share the same pointer.
  - Two atoms created from unequal values have different pointers.
  - `Atom::new` across threads produces consistent results (use `std::thread::spawn`).
  - `Deref` yields the expected value.

Example scaffold:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_value_same_pointer() {
        let a = Atom::new("hello".to_string());
        let b = Atom::new("hello".to_string());
        assert!(std::ptr::eq(&*a as *const str, &*b as *const str));
    }

    #[test]
    fn different_values_different_pointers() {
        let a = Atom::new("foo".to_string());
        let b = Atom::new("bar".to_string());
        assert_ne!(a, b);
    }
}
```

## Building and running the demo

```bash
cargo build          # build the library
cargo run            # run src/main.rs demo
cargo test           # run tests (add some first!)
cargo clippy         # lint
```

## Style conventions

- Keep `lib.rs` self-contained — avoid splitting into modules unless the file grows significantly.
- Prefer `#[inline]` on trivial `Atomize` methods (already done for existing impls).
- All public API items must have doc comments (`///`).
- `main.rs` is a demo only; do not export anything from it.
