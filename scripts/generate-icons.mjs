import sharp from 'sharp';
import pngToIco from 'png-to-ico';
import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, '..');
const iconSrc = join(projectRoot, 'icon.png');
const iconsDir = join(projectRoot, 'src-tauri', 'icons');

const sizes = [32, 64, 128, 256];

async function generateIcons() {
  console.log('Generating icons from:', iconSrc);

  // PNG 생성
  for (const size of sizes) {
    const filename = size === 256 ? '128x128@2x.png' : `${size}x${size}.png`;
    const output = join(iconsDir, filename);
    await sharp(iconSrc)
      .resize(size, size)
      .png()
      .toFile(output);
    console.log(`Created: ${filename}`);
  }

  // icon.png (512x512)
  await sharp(iconSrc)
    .resize(512, 512)
    .png()
    .toFile(join(iconsDir, 'icon.png'));
  console.log('Created: icon.png');

  // ICO 생성 (여러 크기 포함)
  const icoSizes = [16, 24, 32, 48, 64, 128, 256];
  const pngBuffers = await Promise.all(
    icoSizes.map(size =>
      sharp(iconSrc).resize(size, size).png().toBuffer()
    )
  );
  const icoBuffer = await pngToIco(pngBuffers);
  writeFileSync(join(iconsDir, 'icon.ico'), icoBuffer);
  console.log('Created: icon.ico');

  // Windows Store 로고들
  const storeSizes = [
    { name: 'StoreLogo.png', size: 50 },
    { name: 'Square30x30Logo.png', size: 30 },
    { name: 'Square44x44Logo.png', size: 44 },
    { name: 'Square71x71Logo.png', size: 71 },
    { name: 'Square89x89Logo.png', size: 89 },
    { name: 'Square107x107Logo.png', size: 107 },
    { name: 'Square142x142Logo.png', size: 142 },
    { name: 'Square150x150Logo.png', size: 150 },
    { name: 'Square284x284Logo.png', size: 284 },
    { name: 'Square310x310Logo.png', size: 310 },
  ];

  for (const { name, size } of storeSizes) {
    await sharp(iconSrc)
      .resize(size, size)
      .png()
      .toFile(join(iconsDir, name));
    console.log(`Created: ${name}`);
  }

  console.log('\nDone! All icons generated.');
}

generateIcons().catch(console.error);
