# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.2](https://github.com/kantord/headson/compare/headson-v0.11.1...headson-v0.11.2) - 2025-12-16

### Other

- split text ingest pipeline ([#386](https://github.com/kantord/headson/pull/386))
- tidy CLI/debug flow and add CLI golden snapshots ([#384](https://github.com/kantord/headson/pull/384))
- simplify serialization and guard slot stats ([#383](https://github.com/kantord/headson/pull/383))
- stabilize fileset interleave and duplicate penalties ([#381](https://github.com/kantord/headson/pull/381))

## [0.11.1](https://github.com/kantord/headson/compare/headson-v0.11.0...headson-v0.11.1) - 2025-12-15

### Fixed

- fix --tree scaffolding render ([#380](https://github.com/kantord/headson/pull/380))f
- enforce per-file budgets in tree filesets ([#368](https://github.com/kantord/headson/pull/368))

### Other

- budget search
- record fileset tree slot stats in a single render ([#376](https://github.com/kantord/headson/pull/376))
- split serialization renderer into modules ([#367](https://github.com/kantord/headson/pull/367))
- reduce boilerplate in test ([#365](https://github.com/kantord/headson/pull/365))
- small refactors ([#360](https://github.com/kantord/headson/pull/360))
- simplify text omission handling ([#359](https://github.com/kantord/headson/pull/359))
- extract code highlight helpers from renderer ([#358](https://github.com/kantord/headson/pull/358))
- unify render dispatch and tighten length guard ([#356](https://github.com/kantord/headson/pull/356))
- unify string serializer and relax file length cap ([#354](https://github.com/kantord/headson/pull/354))

## [0.11.0](https://github.com/kantord/headson/compare/headson-v0.10.1...headson-v0.11.0) - 2025-12-11

### Added

- stricter enforcement for per-file caps ([#344](https://github.com/kantord/headson/pull/344))

## [0.10.1](https://github.com/kantord/headson/compare/headson-v0.10.0...headson-v0.10.1) - 2025-12-04

### Fixed

- stop cross-file duplicate penalty starving filesets ([#334](https://github.com/kantord/headson/pull/334))

### Other

- deduplicate helpers ([#329](https://github.com/kantord/headson/pull/329))

## [0.10.0](https://github.com/kantord/headson/compare/headson-v0.9.0...headson-v0.10.0) - 2025-12-01

### Added

- add --tree flag ([#326](https://github.com/kantord/headson/pull/326))

## [0.9.0](https://github.com/kantord/headson/compare/headson-v0.8.0...headson-v0.9.0) - 2025-11-29

### Added

- allow showing unmatched files in --grep mode ([#321](https://github.com/kantord/headson/pull/321))
- implement highlighting for grep results ([#313](https://github.com/kantord/headson/pull/313))

### Fixed

- by default, hide files with 0 matches when using --grep ([#317](https://github.com/kantord/headson/pull/317))

### Other

- *(deps)* update rust crate insta to v1.44.3 ([#318](https://github.com/kantord/headson/pull/318))

## [0.8.0](https://github.com/kantord/headson/compare/headson-v0.7.29...headson-v0.8.0) - 2025-11-25

### Added

- implement --grep flag ([#312](https://github.com/kantord/headson/pull/312))

## [0.7.29](https://github.com/kantord/headson/compare/headson-v0.7.28...headson-v0.7.29) - 2025-11-25

### Added

- implement glob feature ([#308](https://github.com/kantord/headson/pull/308))

## [0.7.28](https://github.com/kantord/headson/compare/headson-v0.7.27...headson-v0.7.28) - 2025-11-24

### Fixed

- improve line duplication logic ([#304](https://github.com/kantord/headson/pull/304))

## [0.7.27](https://github.com/kantord/headson/compare/headson-v0.7.26...headson-v0.7.27) - 2025-11-24

### Added

- penalize duplicated code lines ([#300](https://github.com/kantord/headson/pull/300))

## [0.7.26](https://github.com/kantord/headson/compare/headson-v0.7.25...headson-v0.7.26) - 2025-11-24

### Fixed

- improve "prioritize at least one line" logic ([#297](https://github.com/kantord/headson/pull/297))

### Other

- remove a few useless comments ([#295](https://github.com/kantord/headson/pull/295))

## [0.7.25](https://github.com/kantord/headson/compare/headson-v0.7.24...headson-v0.7.25) - 2025-11-24

### Added

- allow optionally counting headers in budgets ([#293](https://github.com/kantord/headson/pull/293))

### Other

- move debug logic to debug.rs ([#292](https://github.com/kantord/headson/pull/292))
- move format detection logic to ingest module ([#291](https://github.com/kantord/headson/pull/291))
- remove textmany ([#290](https://github.com/kantord/headson/pull/290))
- remove some dead code ([#289](https://github.com/kantord/headson/pull/289))
- add textmode ([#288](https://github.com/kantord/headson/pull/288))
- remove redundant input kinds ([#287](https://github.com/kantord/headson/pull/287))
- extract ingest_into_arena ([#286](https://github.com/kantord/headson/pull/286))
- create pruner module ([#285](https://github.com/kantord/headson/pull/285))
- have a single headson function ([#284](https://github.com/kantord/headson/pull/284))
- remove redundant version of headson function ([#282](https://github.com/kantord/headson/pull/282))

## [0.7.24](https://github.com/kantord/headson/compare/headson-v0.7.23...headson-v0.7.24) - 2025-11-23

### Fixed

- fix binary release ([#278](https://github.com/kantord/headson/pull/278))

## [0.7.23](https://github.com/kantord/headson/compare/headson-v0.7.22...headson-v0.7.23) - 2025-11-23

### Fixed

- fix cross-platform build ([#276](https://github.com/kantord/headson/pull/276))

## [0.7.22](https://github.com/kantord/headson/compare/headson-v0.7.21...headson-v0.7.22) - 2025-11-23

### Fixed

- fix binary build

## [0.7.21](https://github.com/kantord/headson/compare/headson-v0.7.20...headson-v0.7.21) - 2025-11-23

### Fixed

- trigger a release ([#272](https://github.com/kantord/headson/pull/272))

## [0.7.20](https://github.com/kantord/headson/compare/headson-v0.7.19...headson-v0.7.20) - 2025-11-23

### Fixed

- fix problems with multi-platform builds ([#267](https://github.com/kantord/headson/pull/267))

## [0.7.19](https://github.com/kantord/headson/compare/headson-v0.7.18...headson-v0.7.19) - 2025-11-23

### Fixed

- fix binary build ([#264](https://github.com/kantord/headson/pull/264))

## [0.7.18](https://github.com/kantord/headson/compare/headson-v0.7.17...headson-v0.7.18) - 2025-11-23

### Fixed

- fix binary releses ([#261](https://github.com/kantord/headson/pull/261))

## [0.7.17](https://github.com/kantord/headson/compare/headson-v0.7.16...headson-v0.7.17) - 2025-11-22

### Fixed

- fix binary build ([#257](https://github.com/kantord/headson/pull/257))

## [0.7.16](https://github.com/kantord/headson/compare/headson-v0.7.15...headson-v0.7.16) - 2025-11-22

### Fixed

- fix binary build matrix ([#255](https://github.com/kantord/headson/pull/255))

## [0.7.15](https://github.com/kantord/headson/compare/headson-v0.7.14...headson-v0.7.15) - 2025-11-22

### Fixed

- fix binary assets ([#253](https://github.com/kantord/headson/pull/253))

## [0.7.14](https://github.com/kantord/headson/compare/headson-v0.7.13...headson-v0.7.14) - 2025-11-22

### Fixed

- fix python build ([#251](https://github.com/kantord/headson/pull/251))

## [0.7.13](https://github.com/kantord/headson/compare/headson-v0.7.12...headson-v0.7.13) - 2025-11-22

### Fixed

- trigger new release ([#249](https://github.com/kantord/headson/pull/249))

## [0.7.12](https://github.com/kantord/headson/compare/headson-v0.7.11...headson-v0.7.12) - 2025-11-22

### Fixed

- fix python build

## [0.7.11](https://github.com/kantord/headson/compare/headson-v0.7.10...headson-v0.7.11) - 2025-11-22

### Fixed

- trigger another release ([#244](https://github.com/kantord/headson/pull/244))

## [0.7.10](https://github.com/kantord/headson/compare/headson-v0.7.9...headson-v0.7.10) - 2025-11-22

### Fixed

- trigger a release ([#242](https://github.com/kantord/headson/pull/242))

## [0.7.9](https://github.com/kantord/headson/compare/headson-v0.7.8...headson-v0.7.9) - 2025-11-22

### Fixed

- trigger a release ([#218](https://github.com/kantord/headson/pull/218))

### Other

- use a single lockfile for python and rust release ([#240](https://github.com/kantord/headson/pull/240))
- *(deps)* update rust crate insta to v1.44.1 ([#234](https://github.com/kantord/headson/pull/234))
- *(deps)* update rust crate insta to v1.44.0 ([#224](https://github.com/kantord/headson/pull/224))
- *(deps)* update rust crate clap to v4.5.53 ([#223](https://github.com/kantord/headson/pull/223))

## [0.7.8](https://github.com/kantord/headson/compare/v0.7.7...v0.7.8) - 2025-11-18

### Other

- Trigger release ([#215](https://github.com/kantord/headson/pull/215))

## [0.7.7](https://github.com/kantord/headson/compare/v0.7.6...v0.7.7) - 2025-11-18

### Fixed

- honor -i input format for single-file auto runs ([#210](https://github.com/kantord/headson/pull/210))

## [0.7.6](https://github.com/kantord/headson/compare/v0.7.5...v0.7.6) - 2025-11-17

### Added

- support syntax highlighting for markdown ([#206](https://github.com/kantord/headson/pull/206))

## [0.7.5](https://github.com/kantord/headson/compare/v0.7.4...v0.7.5) - 2025-11-17

### Other

- set up binary builds for releases ([#201](https://github.com/kantord/headson/pull/201))

## [0.7.4](https://github.com/kantord/headson/compare/v0.7.3...v0.7.4) - 2025-11-17

### Other

- move logic into run.ts ([#185](https://github.com/kantord/headson/pull/185))

## [0.7.3](https://github.com/kantord/headson/compare/v0.7.2...v0.7.3) - 2025-11-17

### Added

- sort files based on git history or edit time ([#181](https://github.com/kantord/headson/pull/181))
- for code files, truncate very long lines ([#179](https://github.com/kantord/headson/pull/179))

### Other

- move cli arg logic to a separate file ([#184](https://github.com/kantord/headson/pull/184))
- move budget related logic to a separate file ([#183](https://github.com/kantord/headson/pull/183))
- improve tape ([#174](https://github.com/kantord/headson/pull/174))

## [0.7.2](https://github.com/kantord/headson/compare/v0.7.1...v0.7.2) - 2025-11-10

### Added

- show more useful lines in code summary ([#172](https://github.com/kantord/headson/pull/172))

### Other

- update demo gif ([#170](https://github.com/kantord/headson/pull/170))

## [0.7.1](https://github.com/kantord/headson/compare/v0.7.0...v0.7.1) - 2025-11-09

### Added

- add --no-header flag ([#168](https://github.com/kantord/headson/pull/168))

### Other

- improve demo gif ([#166](https://github.com/kantord/headson/pull/166))

## [0.7.0](https://github.com/kantord/headson/compare/v0.6.8...v0.7.0) - 2025-11-09

### Added

- [**breaking**] rename CLI binary to hson ([#164](https://github.com/kantord/headson/pull/164))

### Fixed

- ensure tight code filesets show every file ([#162](https://github.com/kantord/headson/pull/162))

## [0.6.8](https://github.com/kantord/headson/compare/v0.6.7...v0.6.8) - 2025-11-08

### Added

- support synax highlighting for code files ([#156](https://github.com/kantord/headson/pull/156))
- smart summary for source code files ([#145](https://github.com/kantord/headson/pull/145))
- properly support multi-format file ingestion ([#153](https://github.com/kantord/headson/pull/153))
- add --debug flag ([#150](https://github.com/kantord/headson/pull/150))

### Fixed

- improve code "parsing" heuristics ([#158](https://github.com/kantord/headson/pull/158))

## [0.6.7](https://github.com/kantord/headson/compare/v0.6.6...v0.6.7) - 2025-11-04

### Other

- fix typo (heal -> head) ([#146](https://github.com/kantord/headson/pull/146))

## [0.6.6](https://github.com/kantord/headson/compare/v0.6.5...v0.6.6) - 2025-11-02

### Added

- add unicode character budget mode ([#140](https://github.com/kantord/headson/pull/140))

## [0.6.5](https://github.com/kantord/headson/compare/v0.6.4...v0.6.5) - 2025-11-02

### Added

- add line based limits ([#137](https://github.com/kantord/headson/pull/137))

### Other

- update demo tape ([#139](https://github.com/kantord/headson/pull/139))
- fix demo gif ([#138](https://github.com/kantord/headson/pull/138))
- fix broken gif ([#135](https://github.com/kantord/headson/pull/135))

## [0.6.4](https://github.com/kantord/headson/compare/v0.6.3...v0.6.4) - 2025-11-02

### Added

- unify character budget syntax with head/tail ([#134](https://github.com/kantord/headson/pull/134))
- unify sampling logic across file formats ([#132](https://github.com/kantord/headson/pull/132))

### Other

- add package version badges ([#131](https://github.com/kantord/headson/pull/131))
- add codecov badge ([#130](https://github.com/kantord/headson/pull/130))
- improve folder structure for ingest logic ([#128](https://github.com/kantord/headson/pull/128))

## [0.6.3](https://github.com/kantord/headson/compare/v0.6.2...v0.6.3) - 2025-11-01

### Added

- support arbitrary text files ([#126](https://github.com/kantord/headson/pull/126))

## [0.6.2](https://github.com/kantord/headson/compare/v0.6.1...v0.6.2) - 2025-11-01

### Added

- separate format/template semantics ([#123](https://github.com/kantord/headson/pull/123))
- use auto format detection when using a single file ([#120](https://github.com/kantord/headson/pull/120))
- automatically pick format based on file extension ([#119](https://github.com/kantord/headson/pull/119))
- add color support for yaml ([#116](https://github.com/kantord/headson/pull/116))
- add yaml loader ([#112](https://github.com/kantord/headson/pull/112))
- add yaml template ([#105](https://github.com/kantord/headson/pull/105))

### Other

- update readme ([#125](https://github.com/kantord/headson/pull/125))
- update demo gif ([#122](https://github.com/kantord/headson/pull/122))
- implement fileset rendering in a single place ([#118](https://github.com/kantord/headson/pull/118))
- *(deps)* update rust crate assert_cmd to v2.1.1 ([#106](https://github.com/kantord/headson/pull/106))
- add snapshot tests for yaml ([#115](https://github.com/kantord/headson/pull/115))
- create common folder for text fixtures ([#113](https://github.com/kantord/headson/pull/113))
- *(deps)* update rust crate clap to v4.5.51 ([#111](https://github.com/kantord/headson/pull/111))
- flatted node structure ([#110](https://github.com/kantord/headson/pull/110))
- add atomic nodes ([#109](https://github.com/kantord/headson/pull/109))

## [0.6.1](https://github.com/kantord/headson/compare/v0.6.0...v0.6.1) - 2025-10-28

### Other

- fix logo height ([#103](https://github.com/kantord/headson/pull/103))
- reduce logo size ([#102](https://github.com/kantord/headson/pull/102))
- improve logo ([#100](https://github.com/kantord/headson/pull/100))

## [0.6.0](https://github.com/kantord/headson/compare/v0.5.4...v0.6.0) - 2025-10-28

### Added

- add color output ([#98](https://github.com/kantord/headson/pull/98))

## [0.5.4](https://github.com/kantord/headson/compare/v0.5.3...v0.5.4) - 2025-10-27

### Other

- fix logo in readme ([#96](https://github.com/kantord/headson/pull/96))
- fix logo ([#95](https://github.com/kantord/headson/pull/95))
- small readme improvements ([#94](https://github.com/kantord/headson/pull/94))
- add terminal gifs ([#92](https://github.com/kantord/headson/pull/92))

## [0.5.3](https://github.com/kantord/headson/compare/v0.5.2...v0.5.3) - 2025-10-26

### Added

- allow global and per-file limits together ([#91](https://github.com/kantord/headson/pull/91))

### Other

- use absolute url for chart ([#89](https://github.com/kantord/headson/pull/89))

## [0.5.2](https://github.com/kantord/headson/compare/v0.5.1...v0.5.2) - 2025-10-26

### Other

- pregenerate mermaid chart ([#87](https://github.com/kantord/headson/pull/87))

## [0.5.1](https://github.com/kantord/headson/compare/v0.5.0...v0.5.1) - 2025-10-26

### Other

- include readme in docs.rs ([#85](https://github.com/kantord/headson/pull/85))

## [0.5.0](https://github.com/kantord/headson/compare/v0.4.0...v0.5.0) - 2025-10-26

### Added

- add --tail flag ([#84](https://github.com/kantord/headson/pull/84))

### Other

- introduce tail sampler ([#81](https://github.com/kantord/headson/pull/81))
- *(ingest)* introduce pluggable array sampler ([#80](https://github.com/kantord/headson/pull/80))
- add support for internal array gaps ([#73](https://github.com/kantord/headson/pull/73))
- add footnotes to algo chart ([#77](https://github.com/kantord/headson/pull/77))
- add mermaid chart ([#75](https://github.com/kantord/headson/pull/75))

## [0.4.0](https://github.com/kantord/headson/compare/v0.3.0...v0.4.0) - 2025-10-26

### Other

- set up abi3-based builds for python bindings ([#70](https://github.com/kantord/headson/pull/70))
- small cleanup ([#69](https://github.com/kantord/headson/pull/69))
- clarify render inclusion semantics and rename confusing fields ([#66](https://github.com/kantord/headson/pull/66))

## [0.3.0](https://github.com/kantord/headson/compare/v0.2.5...v0.3.0) - 2025-10-25

### Added

- add --tail flag ([#58](https://github.com/kantord/headson/pull/58))

### Other

- simplify readme ([#65](https://github.com/kantord/headson/pull/65))
- update features section in README.md ([#64](https://github.com/kantord/headson/pull/64))
- remove links section from README ([#63](https://github.com/kantord/headson/pull/63))
- improve readme ([#33](https://github.com/kantord/headson/pull/33))

## [0.2.5](https://github.com/kantord/headson/compare/v0.2.4...v0.2.5) - 2025-10-25

### Fixed

- *(deps)* update rust crate simd-json to 0.17 ([#50](https://github.com/kantord/headson/pull/50))

### Other

- *(deps)* update rust crate rand to 0.9 ([#40](https://github.com/kantord/headson/pull/40))
- avoid unused code and dependencies ([#43](https://github.com/kantord/headson/pull/43))

## [0.2.4](https://github.com/kantord/headson/compare/v0.2.3...v0.2.4) - 2025-10-25

### Fixed

- ignore binary files ([#34](https://github.com/kantord/headson/pull/34))

## [0.2.3](https://github.com/kantord/headson/compare/v0.2.2...v0.2.3) - 2025-10-25

### Other

- explain basic python usage ([#23](https://github.com/kantord/headson/pull/23))

## [0.2.2](https://github.com/kantord/headson/compare/v0.2.1...v0.2.2) - 2025-10-25

### Other

- update Cargo.lock dependencies

## [0.2.1](https://github.com/kantord/headson/compare/v0.2.0...v0.2.1) - 2025-10-23

### Added

- allow specifying a global character limit ([#13](https://github.com/kantord/headson/pull/13))

## [0.2.0](https://github.com/kantord/headson/compare/v0.1.0...v0.2.0) - 2025-10-23

### Added

- support multiple input files ([#1](https://github.com/kantord/headson/pull/1))
