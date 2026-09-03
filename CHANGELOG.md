# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.1] - 2026-09-03

### Added

- Add an integration test for building nested `#[partial]` structs with
  `emplace_field` directly inside the parent's storage.

### Changed

- Improve the crate-level docs.

## [0.5.0] - 2026-09-03

### Changed

- Drop the `_` prefix from tuple-field method names: `with_0`, `set_0`, `get_0`,
  etc. instead of `with__0`, `set__0`, `get__0`.

## [0.4.0] - 2026-09-03

### Changed

- Improve the field-setting API: rename the field setter from `field(value)` to
  `with_field(value)`, and add `emplace_field(init)` and `uninit_field()` for
  in-place initialization of a field.
- Rename the accessor `get_field_mut()` to `get_mut_field()`, and make
  `set_field()` return `&mut Self` for chaining.

## [0.3.1] - 2026-09-03

### Added

- Add the `CtorComplete<T>` trait and implement it for the generated builder, so
  `done()` can be used generically. It uses a generic type parameter rather than
  an associated type, so implementing the trait never leaks the builder's
  private state through a public trait interface.

### Changed

- The `#[partial]` macro now emits a `CtorComplete` impl for the builder in
  addition to the inherent `done()` method.

## [0.3.0] - 2026-09-02

### Added

- Add build/test scripts (`build.ps1`/`build.sh`, `test.ps1`/`test.sh`) and a `CHANGELOG.md`.
- Add a Clippy CI workflow that enforces `-D warnings`.

### Changed

- Remove the public `PartialThis` trait in favor of an inherent `partial` constructor.
- Refactor the builder to the `Partial<N>` / `ThisPtr` design with a typenum-based bitmask state.
- Emit `PhantomData` only for generic structs.
- Make the generated builder compatible with private field types.
- Fix scope/name collisions and add pub-visibility tests.
- Rename the `UninitThis` trait to `AnyUninit`.
- Mark the `ThisPtr` trait as `#[doc(hidden)]` to simplify the public API.
- Move the proc-macro documentation onto the `partial` re-export so doctests are runnable.

## [0.2.1] - 2026-09-02

### Changed

- Refine the partial-construction API and documentation.

## [0.2.0] - 2026-09-02

### Added

- `#[partial]` attribute macro for type-safe partial construction.
- Support for structs, tuple structs, generics, lifetimes, and private fields.
- Multiple partial sources: `Box`, `MaybeUninit`, and `&mut MaybeUninit`.
- Field initialization and access methods, `done()` finalization, and drop safety.
