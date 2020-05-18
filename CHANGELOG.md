# Changelog

## [0.0.5]

### Added

- rework internals : keep modes in mind
- add crossterm frontend (and use it by default)
- add input map configuration (internal json for now)
- add multi marks, update undo/redo support
- add basic selection
- add basic syntax highlighting
- add word wrapping

### Removed

- remove ncurses frontend


## [0.0.4] 2018-06-03

### Added

- splits the editor in 2 threads
- CHANGELOG.md


## [0.0.3]

### Added

- large file support.<br/>


## [0.0.2]

### Added

- unlimited undo/redo at the byte level:<br/> every inserted character is added to the document's buffer log.<br/>
<br/>
  This is the base api that will be used to implement other undo schemes (word, sentence, paragraph).


## [0.0.1]

* basic utf-8 support
