# Budahbrot renderer

Run the project to open the Iced GUI:

```sh
cargo run
```

To launch the release build directly on the locked Run layout:

```powershell
.\scripts\run-app.ps1
```

The app renders live while worker threads sample the image. The **Gen** tab orders
settings by visual impact: equation power `k`, complex-plane bounds, channel color
mapping, sampling quality, and preview/performance controls.

The Mandelbrot outline is drawn over the render preview using the same preview area
and visible resolution. Dragging a rectangle on the preview selects pending bounds;
those bounds are applied the next time a render starts. Selection is mapped through
the currently displayed render viewport, so selecting inside an already cropped render
crops that visible fractal area again. The outline is hidden while generation is
running. Bounds act as an output crop: orbit seeds are still sampled from the full
default plane, and orbit points outside the crop are skipped without stopping the rest
of the orbit.

Switching to the **Run** tab starts rendering immediately when no render is active.
The **Run** tab has **Play**, **Pause**, **Stop**, and **Save PNG** controls. Saving
opens a native folder picker and writes the current image as `result.png`. The live
gamma slider sits under the render preview. The preview zooms with the mouse wheel
and pans with right- or middle-button drag.

Render workers store raw integer hit counts. The live preview and PNG export apply
brightness scaling from those counts using the current completed sample count and the
selected crop area, so a crop is normalized like a smaller slice of a higher
resolution full-plane render. Gamma is applied live at preview/export time without
restarting the render.

![txt](9k.png "9k.png")
