# Changelog

## [0.1.5](https://github.com/aksiksi/needle/compare/v0.1.4...v0.1.5) (2022-07-31)


### Features

* Add flag to control threading ([4e5571f](https://github.com/aksiksi/needle/commit/4e5571f21e77029386b9e48b83968e71d0a373e2))
* **analyzer:** Add with_* methods for Analyzer config ([e3e8d7e](https://github.com/aksiksi/needle/commit/e3e8d7eb818f233362275c22be9cfbe8d504e154))
* **cli:** Disable skip files by default ([065a212](https://github.com/aksiksi/needle/commit/065a212425ca1dc5dc283cd18abc38968ae16aeb))
* **comparator:** Add run_with_frame_hashes method ([9428225](https://github.com/aksiksi/needle/commit/94282252c8aca5f8550f91c5a0cda98d75d6b0d3))
* **comparator:** Allow conversion from Analyzer to Comparator ([e53c027](https://github.com/aksiksi/needle/commit/e53c027dac1e48db45068cece51a198519fedd78))
* **comparator:** Allow searching for only openings ([34876f4](https://github.com/aksiksi/needle/commit/34876f47b4484d461a2fcb0ddfa99615e578cb84))
* **comparator:** New match search logic ([15d6c3e](https://github.com/aksiksi/needle/commit/15d6c3eb1a5be54450e27787c571ffd7ed4a156f))
* **comparator:** Sort video paths when Comparator is created ([afcf16a](https://github.com/aksiksi/needle/commit/afcf16ae916b99fe391fd106b2c92a44de999c87))
* **needle:** Expose find_video_files function and use it in needle CLI ([84f1f92](https://github.com/aksiksi/needle/commit/84f1f92fc8bddd6de9ff2363875384eb0b73ca97))


### Bug Fixes

* **cli:** Fix video count check ([295fdee](https://github.com/aksiksi/needle/commit/295fdee8cd9d0c6d2b557e24418d27fafee62cd8))
* **comparator:** Allow self comparison during match selection ([af0243e](https://github.com/aksiksi/needle/commit/af0243ea03e074dadb7604f1e6c24c189660d36b))
* **comparator:** Do not pre-allocate heap entries ([65628f4](https://github.com/aksiksi/needle/commit/65628f4dbc35dfa88fedc1c031a179138504f4ab))
* **comparator:** Filter out empty info early ([4e8dc2e](https://github.com/aksiksi/needle/commit/4e8dc2ef69cd5f2c33eb53d56a350784e61aa2c5))
* **comparator:** Load all frame hash data into memory ([d73614a](https://github.com/aksiksi/needle/commit/d73614a48176ceee7e2e845dcf4a8cae2fff29cc))
* **comparator:** Make sure both src and dst are valid during search ([7939d01](https://github.com/aksiksi/needle/commit/7939d01d7d4216ae44556461e7a0f2fd5a171b15))
* **comparator:** Remove processed array during match search ([61ef47b](https://github.com/aksiksi/needle/commit/61ef47b83da6aad0e9d9c89443504d9523a88d6d))
* **comparator:** Run analysis once per file if flag is set ([fc2cec2](https://github.com/aksiksi/needle/commit/fc2cec28fc0a052332c8a8ff0b8b999350f2754f))
* **comparator:** Use a weighted sum of count and duration for match selection ([ccb1959](https://github.com/aksiksi/needle/commit/ccb1959dabcbe0899def6fdac47962538555cbca))
* **comparator:** Use skip files during match selection ([f264b2a](https://github.com/aksiksi/needle/commit/f264b2a1ea025a866cfb14af7c767ae5e629fd26))
* **comparator:** Use video indices instead of paths in all data structures ([8b01540](https://github.com/aksiksi/needle/commit/8b015402344c53b9e73349ad1c10f6705796165e))
* Decrease hash match threshold and increase opening search percentage ([056b461](https://github.com/aksiksi/needle/commit/056b46179fb9f347b51a4eec3a6eae52bfa0257b))


### Miscellaneous

* **comparator:** Fix test ([34d329a](https://github.com/aksiksi/needle/commit/34d329a0f50d5a74af404e68bd225fb52956e2d1))
* **docs:** Basic usage example in library docs ([4cb970c](https://github.com/aksiksi/needle/commit/4cb970ccd105674b329f31f5b97f092e6af5e5f5))
* Use analyze flag in FrameHashes::from_video ([256ca08](https://github.com/aksiksi/needle/commit/256ca08ea201192fc5defa73a9726ddb483b9759))

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
