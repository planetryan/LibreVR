## sorry not all comments in English, I will translate all comments as a TODO item

## DO NOT RUN UNDER WSL !!! WSL DOES NOT SEE USB DEVICES!

## HOW TO RUN:

### install rust/ all the openxr/monando deps

STEP 1: `cargo build --release` or `cargo build` for development

STEP 2: `cargo run`

thats it! should work on a native Linux system with a OpenXR compatible headset

your VR session should initialize and print logs like:

`xr sistema hasieratzen...`

`xr saioa prest`

the program will run a simple frame loop, 
you can extend it with rendering, input logging, or other VR experiments.

#### work in progress, I am working on a driver so it integrates nicely with SteamVR on linux, especially Flatpak or another tricky port