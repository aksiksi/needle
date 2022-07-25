# Changelog

## [0.1.4](https://github.com/aksiksi/needle/compare/v0.1.3...v0.1.4) (2022-07-25)


### Features

* **comparator:** Display info sorted by video filename ([c045ed4](https://github.com/aksiksi/needle/commit/c045ed44a57b167f479257db1376ae35955073f7))
* **comparator:** Return results from run method ([005f4c4](https://github.com/aksiksi/needle/commit/005f4c45562258d1db67c91b07e75c819222efae))
* **needle:** Info subcommand to display FFmpeg version ([791b02e](https://github.com/aksiksi/needle/commit/791b02ee2d0838a00d6da423513430ae15c3692b))


### Bug Fixes

* **comparator:** Adjust opening and ending percentage index ([52692fe](https://github.com/aksiksi/needle/commit/52692fe8ff54fa49fc811a4ba3dc8a0e4dd06e1f))
* Use correct license in Cargo.toml ([7b95d0e](https://github.com/aksiksi/needle/commit/7b95d0e5cf3c9429e28c42cf1dd48b4b9a75b897))


### Miscellaneous

* Add missing docs in both crates ([c9283aa](https://github.com/aksiksi/needle/commit/c9283aa38701a6f2d5e113242c231d651c66603c))
* Docs for util module ([d7b7754](https://github.com/aksiksi/needle/commit/d7b775452bd6cb2bc8c792c9ba4ff5111fd43972))
* Rename compute_video_header_md5sum ([76cbfb7](https://github.com/aksiksi/needle/commit/76cbfb7822bd80cfeee864d8c97c6c488bf6d1e8))

## [0.1.3](https://github.com/aksiksi/needle/compare/v0.1.2...v0.1.3) (2022-07-25)


### Features

* **comparator:** Create a separate flag for skip file creation ([540a4aa](https://github.com/aksiksi/needle/commit/540a4aad8f931bb132ca7736205861b223fdf73e))
* **comparator:** Implement Default for audio::Comparator ([dd18e69](https://github.com/aksiksi/needle/commit/dd18e6934701a7802f588ceb025e8729f01e1136))
* **comparator:** Store and check MD5 hash of video header in skip file ([25b2321](https://github.com/aksiksi/needle/commit/25b232159db9c0ed3a0d2b67ceb3bb5ba8dede32))
* Use with_* API for constructing Comparator ([3b9a496](https://github.com/aksiksi/needle/commit/3b9a4968f003054a6e275ef9dfa61680912f6fc3))


### Bug Fixes

* Move simhash to chromaprint-rust crate ([6e489bb](https://github.com/aksiksi/needle/commit/6e489bbcaa6c4e764257dbad91a7c9f3040ea04f))
* **needle:** Remove InvalidSeekTimestamp error ([aee9e6b](https://github.com/aksiksi/needle/commit/aee9e6b1ad05dbd698720ae05fb73908ac3152ce))
* Setup a workspace ([4482acd](https://github.com/aksiksi/needle/commit/4482acd0fe5ac5d0e921fe945114f62db4bcb21c))


### Miscellaneous

* Add a few more static-related features ([5a35281](https://github.com/aksiksi/needle/commit/5a352817c7fb21932453f754663eeddda0006de8))
* Add needle-capi crate ([4ad69ec](https://github.com/aksiksi/needle/commit/4ad69ecc3531aad8f09560c55dd85a728f8b00ff))
* Added some doc comments ([8b8da8c](https://github.com/aksiksi/needle/commit/8b8da8c76721d3bc2a83dc626734daa4703ffa6a))
* Delete old Cargo.lock ([e2fa1e3](https://github.com/aksiksi/needle/commit/e2fa1e360ec6de484cdeef00cb172ae4f720791c))
* Do not run doctests ([d1c3b13](https://github.com/aksiksi/needle/commit/d1c3b131e263a65c32dadaed566d24e972ddf17a))
* Enable symbol stripping and LTO ([7b65723](https://github.com/aksiksi/needle/commit/7b657239570ea8ada3f0387af67d8a0b4ef07b71))
* Pass MSVC linker flags to rustdoc ([9f9b4a4](https://github.com/aksiksi/needle/commit/9f9b4a48093ec4eb31381ebe61c66542e1fffd1a))
* Removed workspace, now just separate crates ([37aed18](https://github.com/aksiksi/needle/commit/37aed18cde0917b502ca558521d9f7db9c4c4d4c))
