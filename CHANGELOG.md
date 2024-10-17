# Changelog

## [0.0.6]
 - run cargo update
 - fix remove_until_end_of_word/remove_until_char_class_change : add missing undo/redo  operation
 - rework alt+d: (now delete until char class changes)

## [0.0.5]

### Added

- rework internals : add modes
- add crossterm frontend (and use it by default)
- add input map configuration (internal json for now)
- add multi marks, update undo/redo support
- add basic selection
- add basic syntax highlighting
- add word wrapping
- add line numbers
- add goto line
- add basic fin in file (no regex)


### Removed

- remove ncurses frontend
- remove termion frontend

## [0.0.4] 2018-06-03

### Added

- splits the editor in 2 threads
- CHANGELOG.md


## [0.0.3]

### Added

- large file support.<br/>


## [0.0.2]

### Added

- unlimited undo/redo at the byte level:<br/> every inserted character is added to the buffer log.<br/>
<br/>
  This is the base api that will be used to implement other undo schemes (word, sentence, paragraph).


## [0.0.1]

* basic utf-8 support
