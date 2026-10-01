# Lari
This is a Rust implementation of [Conway's Game of Life](https://en.wikipedia.org/wiki/Conway's_Game_of_Life).
It's intended to be a web app that can also run natively on Linux (maybe Windows and macOS later on).
It can also be used as a library.



## Features
- [x] Infinite canvas
- [x] Save state (in base64 using clipboard)
- [x] QOL for the editor
	- [x] Undo and redo
	- [x] Rectangle selection
	- [x] Copy-paste
	- [x] Blueprints
- [ ] Coloring
- [x] Adjustable play rate with pause/unpause
- [ ] Custom rules via rulestrings (i.e. different "universes")
- [ ] Provide library bindings
- [ ] Reverse search (low priority)
