# Lari
A Rust library for simulating
[Conway's Game of Life](https://en.wikipedia.org/wiki/Conway's_Game_of_Life).

This repository also contains the source code of the web app.
With plans to make native apps for Linux, Windows, and maybe macOS.



## Library Features
- [x] Grid serialization
	- [x] [RLE](https://conwaylife.com/wiki/Run_Length_Encoded) support
- [ ] Arbitrary cell data (e.g. coloring info)
- [ ] Custom rules via rulestrings (i.e. different "universes")
- [ ] Reverse search (low priority)



## App Platform Support
- [x] Web
- [ ] Linux
- [ ] Windows
- [ ] macOS

## App Features
- [x] Infinite canvas
- [x] Save state (in base64 using clipboard)
- [x] Undo and redo
- [x] Rectangle selection
- [x] Copy-paste
- [x] Blueprints
- [x] Adjustable play rate with pause/unpause
