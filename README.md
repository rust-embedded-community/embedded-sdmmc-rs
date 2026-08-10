# Embedded SD/MMC

This crate is intended to allow you to read/write files on a FAT formatted SD
card on your Rust Embedded device, as easily as using the `SdFat` Arduino
library. It is written in pure-Rust, is `#![no_std]` and does not use `alloc`
or `collections` to keep the memory footprint low. In the first instance it is
designed for readability and simplicity over performance.

It contans two libraries with dedicated documentation:

- [`embedded-sdmmc`](./embedded-sdmmc/): The main library for users in applications. Contains the
  FAT filesystem stack and the high-level API to read/write files.
- [`embedded-sdmmc-types`](./embedded-sdmmc-types/): Primarily for library authors. Contains
  fundamental types and traits to provide `embedded-sdmmc` support for your SD card interface.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)

- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Copyright notices are stored in the [NOTICE](./NOTICE) file.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

Contributions must be in accordance with the notices in [CONTRIBUTING.md](./CONTRIBUTING.md).
