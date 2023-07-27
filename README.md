# Taiko
(todo: get a more interesting name)
A Taiko no tatsujin clone/simulator. I'm contractually obliged to mention it's written in rust.

This is still under construction. My goals for this project are to create a taiko-style rhythm game that can read and play songs using the tja format. It can... kinda do that now! But there is much work to be done before I am comfortable calling it a "working game".

# Instructions
As there is no release yet, this can only be build with cargo, so install [rust](https://www.rust-lang.org/) and run `cargo run --release` to run the current release version.

The game will automatically read songs from the `songs` directory in the top level of the repository. Any songs should be put in there as a folder containing a .tja file and the audio file referred to by the tja file. The tja file and the directory should have the same name (excluding file extension). For example:

```
taiko/
| src/
| assets/
| songs/
| | My Favourite Song/
| | | My Favourite Song.tja
| | | my_fav_song.ogg
```

## Goals
Current goals
- [x] Parse tja files (ideally, in a way that can efficiently load many songs)
- [x] Create a working prototype that can play basic taiko mode songs
- [ ] Handle input: keyboard/tatacon/general input configuration settings
- [ ] Handle multiplayer

Possible future goals, but not a current priority:
- Skinning
- Port to web
- Read and play osu taiko format
- Create a TJA editor in the game (this would be extremely difficult but sorely needed)
