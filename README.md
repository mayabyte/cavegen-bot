# Cavegen Discord Bot

A simple Discord bot for [Caveripper](https://github.com/mayabyte/caveripper), a Pikmin 2 cave generator and seed finder.

### Building
```bash
cargo build --release
```

### Running
Place your Discord bot token in a text file called `discord_token.txt`.

The bot requires Caveripper's asset and resource folders to run. The resources folder can be acquires from Caveripper's repo, and the assets folder can be extracted using Caveripper's CLI (instructions in its README):
```bash
caveripper extract pikmin2.iso
```

The assets folder must be named `caveripper_assets` for the bot to find it.
