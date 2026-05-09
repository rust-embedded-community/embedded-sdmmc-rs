# Embedded SD/MMC [![crates.io](https://img.shields.io/crates/v/embedded-sdmmc-types.svg)](https://crates.io/crates/embedded-sdmmc-types) [![Documentation](https://docs.rs/embedded-sdmmc-types/badge.svg)](https://docs.rs/embedded-sdmmc-types)

Embedded SD/MMC Types Library
=======

This library contains some common types and abstractions required for implementing
support for `embedded-sdmmc` inside your hardware abstraction layer library.

This also allows HAL library to only depend on this library instead of the higher-level
`embedded-sdmmc` which might have more frequent breaking changes.

Check out the [documentation](https://docs.rs/embedded-sdmmc-types) for more information on the
available types and traits.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)

- MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Copyright notices are stored in the [NOTICE](../NOTICE) file.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

Contributions must be in accordance with the notices in [CONTRIBUTING.md](../CONTRIBUTING.md).
