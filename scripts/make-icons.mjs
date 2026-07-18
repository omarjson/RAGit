// Generates valid PNG + ICO icons for Tauri using sharp, then wraps PNG in a
// correct ICONDIR container. Run: node scripts/make-icons.mjs
import { writeFileSync, mkdirSync } from "node:fs";
import sharp from "sharp";

const base = new URL("../src-tauri/icons/", import.meta.url);
mkdirSync(base, { recursive: true });

const SIZE = 128;

function makeRaw(size) {
  const raw = Buffer.alloc(size * size * 4);
  const cx = size / 2, cy = size / 2, r = size * 0.32;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = (y * size + x) * 4;
      const dx = x - cx + 0.5, dy = y - cy + 0.5;
      const inside = dx * dx + dy * dy <= r * r;
      if (inside) { raw[i]=168; raw[i+1]=85; raw[i+2]=247; raw[i+3]=255; }
      else { raw[i]=24; raw[i+1]=24; raw[i+2]=27; raw[i+3]=255; }
    }
  }
  return raw;
}

// Wrap a PNG buffer in a minimal single-image ICONDIR.
function pngToIco(png, size) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);      // reserved
  header.writeUInt16LE(1, 2);      // type = icon
  header.writeUInt16LE(1, 4);      // count
  const entry = Buffer.alloc(16);
  entry[0] = size; entry[1] = size; entry[2] = 0; entry[3] = 0;
  entry.writeUInt16LE(1, 4);       // planes
  entry.writeUInt16LE(32, 6);      // bpp
  entry.writeUInt32LE(png.length, 8);
  entry.writeUInt32LE(6 + 16, 12); // offset to PNG data
  return Buffer.concat([header, entry, png]);
}

const raw128 = makeRaw(128);
const png128 = await sharp(raw128, { raw: { width: 128, height: 128, channels: 4 } }).png().toBuffer();
const png32 = await sharp(png128).resize(32, 32).png().toBuffer();

writeFileSync(new URL("128x128.png", base), png128);
writeFileSync(new URL("icon.png", base), png128);
writeFileSync(new URL("32x32.png", base), png32);
writeFileSync(new URL("icon.ico", base), pngToIco(png128, 128));
writeFileSync(new URL("icon.icns", base), png128);
console.log("icons written");
