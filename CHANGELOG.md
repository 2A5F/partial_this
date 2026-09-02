# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
