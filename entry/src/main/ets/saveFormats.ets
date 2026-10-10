// Match the desktop host's supported Save As formats. Keep the suggested
// extension first because DocumentViewPicker defaults to the first choice.
export function saveFormatChoices(suggested: string): string[] {
  if (suggested.toLowerCase().endsWith('.pdf')) { return ['PDF 文档|.pdf']; }
  let choices: string[] = [
    'Photoshop PSD|.psd', 'Photoshop PSB|.psb', 'PhotoCraft|.pcraft',
    'PNG 图像|.png', 'JPEG 图像|.jpg', 'WebP 图像|.webp',
    'TIFF 图像|.tif', 'Targa 图像|.tga', 'OpenEXR 图像|.exr'
  ];
  let dot = suggested.lastIndexOf('.');
  let extension = dot >= 0 ? suggested.substring(dot).toLowerCase() : '.psd';
  if (extension === '.jpeg') { extension = '.jpg'; }
  if (extension === '.tiff') { extension = '.tif'; }
  let index = choices.findIndex((choice: string): boolean => choice.endsWith('|' + extension));
  if (index > 0) { choices.unshift(choices.splice(index, 1)[0]); }
  return choices;
}
