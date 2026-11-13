These small fixtures are generated test data, not wallpaper assets.

- `frame.png`: 32 × 32 RGB PNG built with Python's struct/zlib modules.
- `north-east.png`, `south-west.png`: copies written by Apple's ImageIO with
  latitude 50.5, longitude 30.5, N/E or S/W references, and EXIF capture time
  2013-03-05 00:00:00 with an explicit +00:00 offset.
- `oriented.jpg`: 16 × 32 JPEG written by ImageIO with EXIF orientation 6;
  its upright decoded dimensions must be 32 × 16.

Fixtures are committed so running the Rust tests requires no fixture-generation
scripts or Swift compiler.
