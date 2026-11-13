# Third-party notices

TinixCut is a personal, non-distributed application. Code or designs adapted from other
projects are listed here with their original notices so they can be honoured or removed if the
application is ever distributed.

## Clypra (MIT)

Source: https://github.com/AIEraDev/Clypra — Copyright (c) 2026 Clypra Contributors

Used as a reference for:

- `.cube` 3D LUT parsing structure (`src-tauri/src/modules/media/lut.rs`; the TinixCut
  implementation is rewritten to keep full float precision, bound input size and reject
  malformed lines).
- Planned: chroma-key and video-scope algorithms (Phase 58C/58D).

```
MIT License

Copyright (c) 2026 Clypra Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## OpenCut-AI (MIT)

Source: https://github.com/Ekaanth/OpenCut-AI — Copyright 2025 OpenCut-AI

Planned reference for audio cross-correlation clip alignment (Phase 58F).

## Ideas only (no code copied)

- blyacut (GPL-3.0): MCP tool annotations and review-gated destructive edits (Phase 58A).
- Mono Cut (GPL-3.0): edit-command vocabulary comparison.
